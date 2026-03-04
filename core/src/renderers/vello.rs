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

        let (device, queue) = pollster::block_on(adapter.request_device(
            &wgpu::DeviceDescriptor {
                label: Some("chart-device"),
                ..Default::default()
            },
            None,
        ))
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
        };
        surface.configure(&device, &surface_config);

        let vello_renderer = VelloRendererInner::new(
            &device,
            RendererOptions {
                surface_format: Some(format),
                use_cpu: false,
                antialiasing_support: vello::AaSupport::area_only(),
                num_init_threads: None,
            },
        )
        .expect("Failed to create Vello renderer");

        VelloRenderer {
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
        VelloRenderer {
            backend: VelloBackend::new(),
            device,
            queue,
            surface,
            surface_config,
            vello_renderer,
            cached_bottom_scene: None,
        }
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
            Ok(t) => t,
            Err(e) => {
                log::error!("Failed to get surface texture: {}", e);
                return;
            }
        };

        let render_params = vello::RenderParams {
            base_color: vello::peniko::Color::BLACK,
            width: self.surface_config.width,
            height: self.surface_config.height,
            antialiasing_method: AaConfig::Area,
        };

        self.vello_renderer
            .render_to_surface(
                &self.device,
                &self.queue,
                self.backend.scene(),
                &surface_texture,
                &render_params,
            )
            .expect("Vello render failed");

        surface_texture.present();
    }

    fn resize(&mut self, width: u32, height: u32, scale_factor: f64) {
        let physical_width = (width as f64 * scale_factor) as u32;
        let physical_height = (height as f64 * scale_factor) as u32;

        if physical_width > 0 && physical_height > 0 {
            self.surface_config.width = physical_width;
            self.surface_config.height = physical_height;
            self.surface.configure(&self.device, &self.surface_config);
        }
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
}
