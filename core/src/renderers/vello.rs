/// VelloRenderer — owns all wgpu/Vello resources and implements `Renderer`.
///
/// This is the primary GPU-accelerated renderer, using Vello's scene graph
/// and wgpu for cross-platform GPU access (WebGPU, Metal, Vulkan, DX12).
use crate::backends::vello::VelloBackend;
use crate::chart_renderer::{render_bottom_scene, render_crosshair_scene};
use crate::chart_state::ChartState;
use crate::invalidation::InvalidationLevel;
use crate::renderers::Renderer;
use vello::wgpu;
use vello::{AaConfig, Renderer as VelloRendererInner, RendererOptions, Scene};

/// Complete Vello/wgpu rendering pipeline.
///
/// Owns the GPU device, queue, surface, Vello renderer, and cached scene.
/// The `Chart` struct never sees these — it holds a `Box<dyn Renderer>`.
pub struct VelloRenderer {
    pub backend: VelloBackend,
    pub device: wgpu::Device,
    pub queue: wgpu::Queue,
    pub surface: wgpu::Surface<'static>,
    pub surface_config: wgpu::SurfaceConfiguration,
    pub vello_renderer: VelloRendererInner,
    pub cached_bottom_scene: Option<Scene>,
    target_view: wgpu::TextureView,
    blitter: wgpu::util::TextureBlitter,
}

impl VelloRenderer {
    /// Create a VelloRenderer from an existing wgpu instance and surface.
    ///
    /// This performs adapter selection, device creation, surface configuration,
    /// and Vello renderer initialization.
    pub fn new(
        instance: wgpu::Instance,
        surface: wgpu::Surface<'static>,
        width: u32,
        height: u32,
        scale_factor: f64,
    ) -> Self {
        let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
            compatible_surface: Some(&surface),
            power_preference: wgpu::PowerPreference::HighPerformance,
            ..Default::default()
        }))
        .expect("Failed to find a suitable GPU adapter");

        let (device, queue) = pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
            label: Some("chart-device"),
            ..Default::default()
        }))
        .expect("Failed to create GPU device");

        let surface_caps = surface.get_capabilities(&adapter);
        let format = surface_caps
            .formats
            .iter()
            .find(|f| !f.is_srgb())
            .copied()
            .unwrap_or(surface_caps.formats[0]);

        let physical_width = (width as f64 * scale_factor) as u32;
        let physical_height = (height as f64 * scale_factor) as u32;

        let surface_config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format,
            width: physical_width.max(1),
            height: physical_height.max(1),
            present_mode: wgpu::PresentMode::AutoVsync,
            alpha_mode: surface_caps.alpha_modes[0],
            view_formats: vec![],
            desired_maximum_frame_latency: 2,
            color_space: wgpu::SurfaceColorSpace::Auto,
        };
        surface.configure(&device, &surface_config);

        let vello_renderer = VelloRendererInner::new(
            &device,
            RendererOptions {
                use_cpu: false,
                antialiasing_support: vello::AaSupport::area_only(),
                num_init_threads: None,
                pipeline_cache: None,
            },
        )
        .expect("Failed to create Vello renderer");

        let target_view = create_target_view(&device, surface_config.width, surface_config.height);
        let blitter = wgpu::util::TextureBlitter::new(&device, surface_config.format);
        VelloRenderer {
            target_view,
            blitter,
            backend: VelloBackend::new(),
            device,
            queue,
            surface,
            surface_config,
            vello_renderer,
            cached_bottom_scene: None,
        }
    }

    /// Create a VelloRenderer for WASM with pre-created device/queue/surface/renderer.
    #[cfg(target_arch = "wasm32")]
    pub fn new_from_parts(
        device: wgpu::Device,
        queue: wgpu::Queue,
        surface: wgpu::Surface<'static>,
        surface_config: wgpu::SurfaceConfiguration,
        vello_renderer: VelloRendererInner,
    ) -> Self {
        let target_view = create_target_view(&device, surface_config.width, surface_config.height);
        let blitter = wgpu::util::TextureBlitter::new(&device, surface_config.format);
        VelloRenderer {
            target_view,
            blitter,
            backend: VelloBackend::new(),
            device,
            queue,
            surface,
            surface_config,
            vello_renderer,
            cached_bottom_scene: None,
        }
    }

    /// Render the current scene, then copy it to the acquired surface view.
    /// Vello's compute output requires an RGBA storage texture; window surfaces
    /// generally only support render attachments (and may use BGRA).
    pub fn render_to_view(&mut self, surface_view: &wgpu::TextureView) {
        let params = vello::RenderParams {
            base_color: vello::peniko::Color::BLACK,
            width: self.surface_config.width,
            height: self.surface_config.height,
            antialiasing_method: AaConfig::Area,
        };
        self.vello_renderer
            .render_to_texture(
                &self.device,
                &self.queue,
                self.backend.scene(),
                &self.target_view,
                &params,
            )
            .expect("Vello render failed");
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("chart-blit"),
            });
        self.blitter
            .copy(&self.device, &mut encoder, &self.target_view, surface_view);
        self.queue.submit([encoder.finish()]);
    }
}

impl Renderer for VelloRenderer {
    fn render(&mut self, state: &mut ChartState, level: InvalidationLevel) {
        self.backend.reset();

        if level.needs_bottom_scene() {
            // Light or Full — rebuild the bottom scene via backend
            let mut bottom_backend = VelloBackend::new();
            render_bottom_scene(&mut bottom_backend, state);
            let bottom_scene = bottom_backend.scene;
            self.backend.scene_mut().append(&bottom_scene, None);
            self.cached_bottom_scene = Some(bottom_scene);
            state.bottom_render_count += 1;
        } else if let Some(ref cached) = self.cached_bottom_scene {
            // Cursor only — reuse cached bottom scene
            self.backend.scene_mut().append(cached, None);
        } else {
            // No cache yet — must do full render
            let mut bottom_backend = VelloBackend::new();
            render_bottom_scene(&mut bottom_backend, state);
            let bottom_scene = bottom_backend.scene;
            self.backend.scene_mut().append(&bottom_scene, None);
            self.cached_bottom_scene = Some(bottom_scene);
            state.bottom_render_count += 1;
        }

        // Always render crosshair on top
        render_crosshair_scene(&mut self.backend, state);
        state.crosshair_render_count += 1;

        let surface_texture = match self.surface.get_current_texture() {
            wgpu::CurrentSurfaceTexture::Success(t)
            | wgpu::CurrentSurfaceTexture::Suboptimal(t) => t,
            e => {
                log::error!("Failed to get surface texture: {:?}", e);
                return;
            }
        };

        let surface_view = surface_texture
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());
        self.render_to_view(&surface_view);

        self.queue.present(surface_texture);
    }

    fn resize(&mut self, width: u32, height: u32, scale_factor: f64) {
        let physical_width = (width as f64 * scale_factor) as u32;
        let physical_height = (height as f64 * scale_factor) as u32;

        if physical_width > 0 && physical_height > 0 {
            self.surface_config.width = physical_width;
            self.surface_config.height = physical_height;
            self.surface.configure(&self.device, &self.surface_config);
            self.target_view = create_target_view(&self.device, physical_width, physical_height);
        }
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
}

fn create_target_view(device: &wgpu::Device, width: u32, height: u32) -> wgpu::TextureView {
    device
        .create_texture(&wgpu::TextureDescriptor {
            label: Some("chart-vello-target"),
            size: wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8Unorm,
            usage: wgpu::TextureUsages::STORAGE_BINDING | wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        })
        .create_view(&wgpu::TextureViewDescriptor::default())
}
