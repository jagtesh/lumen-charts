use wasm_bindgen::prelude::*;
use vello::wgpu;
use vello::{AaConfig, Renderer as VelloRenderer, RendererOptions, Scene};

use chart_core::chart_model::{ChartData, ChartLayout};
use chart_core::chart_renderer::render_chart;
use chart_core::sample_data::sample_data;

#[wasm_bindgen(start)]
pub async fn main() {
    console_error_panic_hook::set_once();
    console_log::init_with_level(log::Level::Info).ok();

    log::info!("Chart WASM starting...");

    // Get the canvas element
    let window = web_sys::window().unwrap();
    let document = window.document().unwrap();
    let canvas = document
        .get_element_by_id("chart-canvas")
        .unwrap()
        .dyn_into::<web_sys::HtmlCanvasElement>()
        .unwrap();

    let width = canvas.client_width() as u32;
    let height = canvas.client_height() as u32;
    let scale_factor = window.device_pixel_ratio();

    let physical_width = (width as f64 * scale_factor) as u32;
    let physical_height = (height as f64 * scale_factor) as u32;
    canvas.set_width(physical_width);
    canvas.set_height(physical_height);

    // Create wgpu instance (WebGPU backend in browser)
    let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
        backends: wgpu::Backends::BROWSER_WEBGPU,
        ..Default::default()
    });

    // Create surface from canvas
    let surface = instance
        .create_surface(wgpu::SurfaceTarget::Canvas(canvas.clone()))
        .expect("Failed to create surface from canvas");

    // Get adapter + device
    let adapter = instance
        .request_adapter(&wgpu::RequestAdapterOptions {
            compatible_surface: Some(&surface),
            power_preference: wgpu::PowerPreference::HighPerformance,
            ..Default::default()
        })
        .await
        .expect("No WebGPU adapter found");

    let (device, queue) = adapter
        .request_device(
            &wgpu::DeviceDescriptor {
                label: Some("chart-wasm-device"),
                ..Default::default()
            },
            None,
        )
        .await
        .expect("Failed to create device");

    // Configure surface
    let surface_caps = surface.get_capabilities(&adapter);
    let format = surface_caps
        .formats
        .iter()
        .find(|f| !f.is_srgb())
        .copied()
        .unwrap_or(surface_caps.formats[0]);

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
    let mut vello_renderer = VelloRenderer::new(
        &device,
        RendererOptions {
            surface_format: Some(format),
            use_cpu: false,
            antialiasing_support: vello::AaSupport::area_only(),
            num_init_threads: None,
        },
    )
    .expect("Failed to create Vello renderer");

    // Build chart data and scene
    let data = ChartData {
        bars: sample_data(),
    };
    let layout = ChartLayout::new(width as f32, height as f32, scale_factor);

    let mut scene = Scene::new();
    render_chart(&mut scene, &data, &layout);

    // Render
    let surface_texture = surface
        .get_current_texture()
        .expect("Failed to get surface texture");

    let render_params = vello::RenderParams {
        base_color: vello::peniko::Color::BLACK,
        width: physical_width,
        height: physical_height,
        antialiasing_method: AaConfig::Area,
    };

    vello_renderer
        .render_to_surface(&device, &queue, &scene, &surface_texture, &render_params)
        .expect("Vello render failed");

    surface_texture.present();

    log::info!("Chart rendered successfully!");
}
