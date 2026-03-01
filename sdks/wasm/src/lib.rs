//! WASM SDK — zero-cost passthrough to C-ABI functions.
//!
//! Each wasm_bindgen function is a thin wrapper that calls the corresponding
//! C-ABI function from the core library. This eliminates code duplication
//! and automatically gets invalidation, scene caching, and all new APIs.

use wasm_bindgen::prelude::*;
use vello::wgpu;
use vello::{AaConfig, Renderer as VelloRenderer, RendererOptions, Scene};

use lumen_charts::Chart;
use lumen_charts::chart_model::ChartData;
use lumen_charts::chart_state::ChartState;

// Single global chart pointer (WASM is single-threaded, no need for thread_local)
static mut CHART_PTR: *mut Chart = std::ptr::null_mut();

/// Get the chart pointer, panicking if not initialized.
#[inline(always)]
fn ptr() -> *mut Chart {
    unsafe {
        assert!(!CHART_PTR.is_null(), "Chart not initialized");
        CHART_PTR
    }
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

    // Generate sample data
    let bars = generate_sample_bars();
    log::info!("Generated {} sample bars", bars.len());

    let data = ChartData { bars };
    let state = ChartState::new(data, width as f32, height as f32, scale_factor);

    let chart = Chart {
        state,
        scene: Scene::new(),
        device,
        queue,
        surface,
        surface_config,
        vello_renderer,
        cached_bottom_scene: None,
        click_cb: None,
        crosshair_move_cb: None,
        dbl_click_cb: None,
    };

    // Store as raw pointer
    let chart_ptr = Box::into_raw(Box::new(chart));
    unsafe { CHART_PTR = chart_ptr; }

    // Fit content + render
    lumen_charts::chart_fit_content(chart_ptr);
    lumen_charts::chart_render(chart_ptr);

    log::info!("Chart rendered and interactive!");
}

// === Rendering ===

#[wasm_bindgen]
pub fn chart_render() { lumen_charts::chart_render(ptr()); }

#[wasm_bindgen]
pub fn chart_render_if_needed() -> bool { lumen_charts::chart_render_if_needed(ptr()) }

// === Interactions ===

#[wasm_bindgen]
pub fn chart_pointer_move(x: f32, y: f32) -> bool { lumen_charts::chart_pointer_move(ptr(), x, y) }

#[wasm_bindgen]
pub fn chart_pointer_down(x: f32, y: f32) -> bool { lumen_charts::chart_pointer_down(ptr(), x, y, 0) }

#[wasm_bindgen]
pub fn chart_pointer_up(x: f32, y: f32) -> bool { lumen_charts::chart_pointer_up(ptr(), x, y, 0) }

#[wasm_bindgen]
pub fn chart_pointer_leave() -> bool { lumen_charts::chart_pointer_leave(ptr()) }

#[wasm_bindgen]
pub fn chart_scroll(dx: f32, dy: f32) -> bool { lumen_charts::chart_scroll(ptr(), dx, dy) }

#[wasm_bindgen]
pub fn chart_zoom(factor: f32, cx: f32) -> bool { lumen_charts::chart_zoom(ptr(), factor, cx) }

#[wasm_bindgen]
pub fn chart_pinch(scale: f32, cx: f32, cy: f32) -> bool { lumen_charts::chart_pinch(ptr(), scale, cx, cy) }

#[wasm_bindgen]
pub fn chart_fit_content() -> bool { lumen_charts::chart_fit_content(ptr()) }

#[wasm_bindgen]
pub fn chart_key_down(key_code: u32) -> bool { lumen_charts::chart_key_down(ptr(), key_code) }

#[wasm_bindgen]
pub fn chart_tick() { lumen_charts::chart_tick(ptr()); }

// === Touch events ===

#[wasm_bindgen]
pub fn chart_touch_start(id: u32, x: f32, y: f32) -> bool { lumen_charts::chart_touch_start(ptr(), id, x, y) }

#[wasm_bindgen]
pub fn chart_touch_move(id: u32, x: f32, y: f32) -> bool { lumen_charts::chart_touch_move(ptr(), id, x, y) }

#[wasm_bindgen]
pub fn chart_touch_end(id: u32) -> bool { lumen_charts::chart_touch_end(ptr(), id) }

#[wasm_bindgen]
pub fn chart_touch_tick() { lumen_charts::chart_touch_tick(ptr()); }

// === Data management ===

#[wasm_bindgen]
pub fn chart_set_data(data: &[f64]) {
    let count = data.len() / 5;
    let flat = data.as_ptr();
    // C-ABI expects *const f64 and count
    lumen_charts::chart_set_data(ptr(), flat, count as u32);
}

#[wasm_bindgen]
pub fn chart_set_series_type(t: u32) -> bool { lumen_charts::chart_set_series_type(ptr(), t) }

#[wasm_bindgen]
pub fn chart_bar_count() -> u32 { lumen_charts::chart_bar_count(ptr()) }

// === Series management ===

#[wasm_bindgen]
pub fn chart_add_line_series(data: &[f64]) -> u32 {
    let count = (data.len() / 2) as u32;
    let times: Vec<i64> = (0..count as usize).map(|i| data[i * 2] as i64).collect();
    let values: Vec<f64> = (0..count as usize).map(|i| data[i * 2 + 1]).collect();
    lumen_charts::chart_add_line_series(ptr(), times.as_ptr(), values.as_ptr(), count)
}

#[wasm_bindgen]
pub fn chart_add_area_series(data: &[f64]) -> u32 {
    let count = (data.len() / 2) as u32;
    let times: Vec<i64> = (0..count as usize).map(|i| data[i * 2] as i64).collect();
    let values: Vec<f64> = (0..count as usize).map(|i| data[i * 2 + 1]).collect();
    lumen_charts::chart_add_area_series(ptr(), times.as_ptr(), values.as_ptr(), count)
}

#[wasm_bindgen]
pub fn chart_add_histogram_series(data: &[f64]) -> u32 {
    let count = (data.len() / 2) as u32;
    let times: Vec<i64> = (0..count as usize).map(|i| data[i * 2] as i64).collect();
    let values: Vec<f64> = (0..count as usize).map(|i| data[i * 2 + 1]).collect();
    lumen_charts::chart_add_histogram_series(
        ptr(), times.as_ptr(), values.as_ptr(), std::ptr::null(), count,
    )
}

#[wasm_bindgen]
pub fn chart_add_ohlc_series(data: &[f64]) -> u32 {
    let count = (data.len() / 5) as u32;
    let times: Vec<i64> = (0..count as usize).map(|i| data[i * 5] as i64).collect();
    let opens: Vec<f64> = (0..count as usize).map(|i| data[i * 5 + 1]).collect();
    let highs: Vec<f64> = (0..count as usize).map(|i| data[i * 5 + 2]).collect();
    let lows: Vec<f64> = (0..count as usize).map(|i| data[i * 5 + 3]).collect();
    let closes: Vec<f64> = (0..count as usize).map(|i| data[i * 5 + 4]).collect();
    lumen_charts::chart_add_ohlc_series(
        ptr(), times.as_ptr(), opens.as_ptr(), highs.as_ptr(), lows.as_ptr(), closes.as_ptr(), count,
    )
}

#[wasm_bindgen]
pub fn chart_add_candlestick_series(data: &[f64]) -> u32 {
    let count = (data.len() / 5) as u32;
    let times: Vec<i64> = (0..count as usize).map(|i| data[i * 5] as i64).collect();
    let opens: Vec<f64> = (0..count as usize).map(|i| data[i * 5 + 1]).collect();
    let highs: Vec<f64> = (0..count as usize).map(|i| data[i * 5 + 2]).collect();
    let lows: Vec<f64> = (0..count as usize).map(|i| data[i * 5 + 3]).collect();
    let closes: Vec<f64> = (0..count as usize).map(|i| data[i * 5 + 4]).collect();
    lumen_charts::chart_add_candlestick_series(
        ptr(), times.as_ptr(), opens.as_ptr(), highs.as_ptr(), lows.as_ptr(), closes.as_ptr(), count,
    )
}

#[wasm_bindgen]
pub fn chart_remove_series(id: u32) -> bool { lumen_charts::chart_remove_series(ptr(), id) }

#[wasm_bindgen]
pub fn chart_series_count() -> u32 { lumen_charts::chart_series_count(ptr()) }

// === Options ===

#[wasm_bindgen]
pub fn chart_apply_options(json: &str) -> bool {
    let cstr = std::ffi::CString::new(json).unwrap();
    lumen_charts::chart_apply_options(ptr(), cstr.as_ptr())
}

// === Coordinate translation ===

#[wasm_bindgen]
pub fn chart_price_to_coordinate(price: f64) -> f32 { lumen_charts::chart_price_to_coordinate(ptr(), price) }

#[wasm_bindgen]
pub fn chart_coordinate_to_price(y: f32) -> f64 { lumen_charts::chart_coordinate_to_price(ptr(), y) }

#[wasm_bindgen]
pub fn chart_time_to_coordinate(time: f64) -> f32 { lumen_charts::chart_time_to_coordinate(ptr(), time as i64) }

// === ITimeScaleApi ===

#[wasm_bindgen]
pub fn chart_time_scale_scroll_to_position(pos: f32) { lumen_charts::chart_time_scale_scroll_to_position(ptr(), pos, false); }

#[wasm_bindgen]
pub fn chart_time_scale_scroll_to_real_time() { lumen_charts::chart_time_scale_scroll_to_real_time(ptr()); }

#[wasm_bindgen]
pub fn chart_time_scale_reset() { lumen_charts::chart_time_scale_reset(ptr()); }

#[wasm_bindgen]
pub fn chart_time_scale_width() -> f32 { lumen_charts::chart_time_scale_width(ptr()) }

#[wasm_bindgen]
pub fn chart_time_scale_height() -> f32 { lumen_charts::chart_time_scale_height(ptr()) }

// === IPriceScaleApi ===

#[wasm_bindgen]
pub fn chart_price_scale_get_mode() -> u32 { lumen_charts::chart_price_scale_get_mode(ptr()) }

#[wasm_bindgen]
pub fn chart_price_scale_set_mode(mode: u32) { lumen_charts::chart_price_scale_set_mode(ptr(), mode); }

// === Resize ===

#[wasm_bindgen]
pub fn chart_resize(width: u32, height: u32, scale_factor: f64) {
    lumen_charts::chart_resize(ptr(), width, height, scale_factor);
}

// === Sample data generator ===

fn generate_sample_bars() -> Vec<lumen_charts::chart_model::OhlcBar> {
    let mut bars = Vec::with_capacity(100);
    let base_time: i64 = 1704153600;
    let day: i64 = 86400;
    let mut price: f64 = 185.0;
    let mut rng: u64 = 42;

    for i in 0..100 {
        rng = rng.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
        let r1 = (rng >> 33) as f64 / (1u64 << 31) as f64 * 2.0 - 1.0;
        rng = rng.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
        let r2 = (rng >> 33) as f64 / (1u64 << 31) as f64 * 2.0 - 1.0;
        rng = rng.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
        let r3 = (rng >> 33) as f64 / (1u64 << 31) as f64 * 2.0 - 1.0;

        let change_pct = r1 * 0.02;
        let daily_range = price * (0.005 + r2.abs() * 0.015);
        let open = price;
        let close = price * (1.0 + change_pct);
        let high = open.max(close) + daily_range * r3.abs();
        let low = open.min(close) - daily_range * r2.abs();

        bars.push(lumen_charts::chart_model::OhlcBar {
            time: base_time + i * day,
            open,
            high: high.max(open.max(close)),
            low: low.min(open.min(close)),
            close,
        });
        price = close;
    }
    bars
}
