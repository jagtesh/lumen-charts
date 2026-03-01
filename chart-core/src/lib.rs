pub mod chart_model;
pub mod chart_renderer;
pub mod price_scale;
pub mod sample_data;
pub mod tick_marks;
pub mod time_scale;

use std::ffi::c_void;

use chart_model::{ChartData, ChartLayout};
use chart_renderer::render_chart;
use sample_data::sample_data;

// Use Vello's re-exported wgpu to avoid version conflicts
use vello::wgpu;
use vello::{AaConfig, Renderer as VelloRenderer, RendererOptions, Scene};

/// Opaque chart handle passed to Swift via C-ABI
pub struct Chart {
    data: ChartData,
    width: u32,
    height: u32,
    scale_factor: f64,
    scene: Scene,

    // wgpu + Vello rendering state
    device: wgpu::Device,
    queue: wgpu::Queue,
    surface: wgpu::Surface<'static>,
    surface_config: wgpu::SurfaceConfiguration,
    vello_renderer: VelloRenderer,
}

// ---------------------------------------------------------------------------
// C-ABI Exports
// ---------------------------------------------------------------------------

/// Create a new chart instance. `metal_layer` must be a pointer to a
/// `CAMetalLayer` on macOS. Returns an opaque handle.
#[no_mangle]
pub extern "C" fn chart_create(
    width: u32,
    height: u32,
    scale_factor: f64,
    metal_layer: *mut c_void,
) -> *mut Chart {
    env_logger::try_init().ok();

    let data = ChartData {
        bars: sample_data(),
    };

    // Create wgpu instance
    let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
        backends: wgpu::Backends::METAL,
        ..Default::default()
    });

    // Create surface from CAMetalLayer
    let surface = unsafe {
        instance
            .create_surface_unsafe(wgpu::SurfaceTargetUnsafe::CoreAnimationLayer(metal_layer))
            .expect("Failed to create wgpu surface from CAMetalLayer")
    };

    // Get adapter
    let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
        compatible_surface: Some(&surface),
        power_preference: wgpu::PowerPreference::HighPerformance,
        ..Default::default()
    }))
    .expect("Failed to find a suitable GPU adapter");

    // Request device and queue (wgpu 23 API: 2 args, not 3)
    let (device, queue) = pollster::block_on(adapter.request_device(
        &wgpu::DeviceDescriptor {
            label: Some("chart-device"),
            ..Default::default()
        },
        None,
    ))
    .expect("Failed to create GPU device");

    // Configure surface
    let surface_caps = surface.get_capabilities(&adapter);
    // Vello handles sRGB conversion internally — it rejects sRGB surface formats.
    // Use a non-sRGB format (prefer Bgra8Unorm which matches CAMetalLayer default).
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

    // Create Vello renderer
    let vello_renderer = VelloRenderer::new(
        &device,
        RendererOptions {
            surface_format: Some(format),
            use_cpu: false,
            antialiasing_support: vello::AaSupport::area_only(),
            num_init_threads: None,
        },
    )
    .expect("Failed to create Vello renderer");

    let chart = Chart {
        data,
        width,
        height,
        scale_factor,
        scene: Scene::new(),
        device,
        queue,
        surface,
        surface_config,
        vello_renderer,
    };

    Box::into_raw(Box::new(chart))
}

/// Render the chart. Call this when the view needs a redraw.
#[no_mangle]
pub extern "C" fn chart_render(chart: *mut Chart) {
    let chart = unsafe {
        assert!(!chart.is_null());
        &mut *chart
    };

    // Build the scene
    chart.scene.reset();
    let layout = ChartLayout::new(chart.width as f32, chart.height as f32, chart.scale_factor);
    render_chart(&mut chart.scene, &chart.data, &layout);

    // Get surface texture
    let surface_texture = match chart.surface.get_current_texture() {
        Ok(t) => t,
        Err(e) => {
            log::error!("Failed to get surface texture: {}", e);
            return;
        }
    };

    let render_params = vello::RenderParams {
        base_color: vello::peniko::Color::BLACK,
        width: chart.surface_config.width,
        height: chart.surface_config.height,
        antialiasing_method: AaConfig::Area,
    };

    // Render scene to surface
    chart
        .vello_renderer
        .render_to_surface(
            &chart.device,
            &chart.queue,
            &chart.scene,
            &surface_texture,
            &render_params,
        )
        .expect("Vello render failed");

    surface_texture.present();
}

/// Resize the chart. Call when the window/view size changes.
#[no_mangle]
pub extern "C" fn chart_resize(chart: *mut Chart, width: u32, height: u32, scale_factor: f64) {
    let chart = unsafe {
        assert!(!chart.is_null());
        &mut *chart
    };

    chart.width = width;
    chart.height = height;
    chart.scale_factor = scale_factor;

    let physical_width = (width as f64 * scale_factor) as u32;
    let physical_height = (height as f64 * scale_factor) as u32;

    if physical_width > 0 && physical_height > 0 {
        chart.surface_config.width = physical_width;
        chart.surface_config.height = physical_height;
        chart
            .surface
            .configure(&chart.device, &chart.surface_config);
    }
}

/// Destroy the chart and free all resources.
#[no_mangle]
pub extern "C" fn chart_destroy(chart: *mut Chart) {
    if !chart.is_null() {
        unsafe {
            drop(Box::from_raw(chart));
        }
    }
}
