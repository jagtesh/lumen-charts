use wasm_bindgen::prelude::*;
use vello::wgpu;
use vello::{AaConfig, Renderer as VelloRenderer, RendererOptions, Scene};
use std::cell::RefCell;
use std::rc::Rc;

use chart_core::chart_model::ChartData;
use chart_core::chart_renderer::render_chart;
use chart_core::chart_state::ChartState;
use chart_core::sample_data::sample_data;

/// Persistent chart context for the WASM module
struct WasmChart {
    state: ChartState,
    scene: Scene,
    device: wgpu::Device,
    queue: wgpu::Queue,
    surface: wgpu::Surface<'static>,
    surface_config: wgpu::SurfaceConfiguration,
    vello_renderer: VelloRenderer,
}

impl WasmChart {
    fn render(&mut self) {
        self.scene.reset();
        render_chart(&mut self.scene, &self.state);

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
                &self.scene,
                &surface_texture,
                &render_params,
            )
            .expect("Vello render failed");

        surface_texture.present();
    }
}

// Thread-local persistent chart instance
thread_local! {
    static CHART: RefCell<Option<WasmChart>> = RefCell::new(None);
}

fn with_chart<F: FnOnce(&mut WasmChart) -> bool>(f: F) {
    CHART.with(|c| {
        if let Some(chart) = c.borrow_mut().as_mut() {
            if f(chart) {
                chart.render();
            }
        }
    });
}

#[wasm_bindgen(start)]
pub async fn main() {
    console_error_panic_hook::set_once();
    console_log::init_with_level(log::Level::Info).ok();

    log::info!("Chart WASM starting...");

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

    let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
        backends: wgpu::Backends::BROWSER_WEBGPU,
        ..Default::default()
    });

    let surface = instance
        .create_surface(wgpu::SurfaceTarget::Canvas(canvas.clone()))
        .expect("Failed to create surface from canvas");

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

    let data = ChartData {
        bars: sample_data(),
    };
    let state = ChartState::new(data, width as f32, height as f32, scale_factor);

    let mut chart = WasmChart {
        state,
        scene: Scene::new(),
        device,
        queue,
        surface,
        surface_config,
        vello_renderer,
    };

    // Initial render
    chart.render();

    // Store persistently
    CHART.with(|c| {
        *c.borrow_mut() = Some(chart);
    });

    log::info!("Chart rendered and interactive!");
}

// --- Exported interaction functions ---

#[wasm_bindgen]
pub fn chart_pointer_move(x: f32, y: f32) {
    with_chart(|chart| chart.state.pointer_move(x, y));
}

#[wasm_bindgen]
pub fn chart_pointer_down(x: f32, y: f32) {
    with_chart(|chart| chart.state.pointer_down(x, y, 0));
}

#[wasm_bindgen]
pub fn chart_pointer_up(x: f32, y: f32) {
    with_chart(|chart| chart.state.pointer_up(x, y, 0));
}

#[wasm_bindgen]
pub fn chart_pointer_leave() {
    with_chart(|chart| chart.state.pointer_leave());
}

#[wasm_bindgen]
pub fn chart_scroll(delta_x: f32, delta_y: f32) {
    with_chart(|chart| chart.state.scroll(delta_x, delta_y));
}

#[wasm_bindgen]
pub fn chart_zoom(factor: f32, center_x: f32) {
    with_chart(|chart| chart.state.zoom(factor, center_x));
}

#[wasm_bindgen]
pub fn chart_fit_content() {
    with_chart(|chart| chart.state.fit_content());
}
