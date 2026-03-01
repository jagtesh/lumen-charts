use wasm_bindgen::prelude::*;
use vello::wgpu;
use vello::{AaConfig, Renderer as VelloRenderer, RendererOptions, Scene};
use std::cell::RefCell;

use chart_core::chart_model::ChartData;
use chart_core::chart_renderer::render_chart;
use chart_core::chart_state::ChartState;

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

    // Start with empty data — host JS calls chart_set_data() to load bars
    let data = ChartData { bars: Vec::new() };
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

#[wasm_bindgen]
pub fn chart_key_down(key_code: u32) {
    with_chart(|chart| {
        let key = chart_core::chart_state::ChartKey::from_code(key_code);
        chart.state.key_down(key)
    });
}

#[wasm_bindgen]
pub fn chart_tick() {
    CHART.with(|c| {
        if let Some(chart) = c.borrow_mut().as_mut() {
            chart.state.tick();
        }
    });
}

/// Set bar data from a flat Float64Array: [time, open, high, low, close, ...]
#[wasm_bindgen]
pub fn chart_set_data(data: &[f64]) {
    let count = data.len() / 5;
    let bars: Vec<chart_core::chart_model::OhlcBar> = (0..count)
        .map(|i| {
            let base = i * 5;
            chart_core::chart_model::OhlcBar {
                time: data[base] as i64,
                open: data[base + 1],
                high: data[base + 2],
                low: data[base + 3],
                close: data[base + 4],
            }
        })
        .collect();

    with_chart(|chart| {
        chart.state.set_data(bars);
        true
    });
}

/// Set series type: 0=OHLC, 1=Candlestick, 2=Line
#[wasm_bindgen]
pub fn chart_set_series_type(series_type: u32) {
    with_chart(|chart| {
        chart.state.active_series_type = match series_type {
            0 => chart_core::series::SeriesType::Ohlc,
            1 => chart_core::series::SeriesType::Candlestick,
            2 => chart_core::series::SeriesType::Line,
            _ => return false,
        };
        true
    });
}

// --- Helper for functions that return values ---

fn with_chart_ret<T: Default, F: FnOnce(&mut WasmChart) -> T>(f: F) -> T {
    CHART.with(|c| {
        if let Some(chart) = c.borrow_mut().as_mut() {
            f(chart)
        } else {
            T::default()
        }
    })
}

// --- Series management ---

/// Add an OHLC series. Data: flat [time, open, high, low, close, ...]. Returns series ID.
#[wasm_bindgen]
pub fn chart_add_ohlc_series(data: &[f64]) -> u32 {
    let bars = parse_ohlc_data(data);
    with_chart_ret(|chart| {
        let id = chart.state.series.add(chart_core::series::Series::ohlc(0, bars));
        chart.state.update_price_scale();
        chart.render();
        id
    })
}

/// Add a candlestick series. Data: flat [time, open, high, low, close, ...]. Returns series ID.
#[wasm_bindgen]
pub fn chart_add_candlestick_series(data: &[f64]) -> u32 {
    let bars = parse_ohlc_data(data);
    with_chart_ret(|chart| {
        let id = chart.state.series.add(chart_core::series::Series::candlestick(0, bars));
        chart.state.update_price_scale();
        chart.render();
        id
    })
}

/// Add a line series. Data: flat [time, value, ...]. Returns series ID.
#[wasm_bindgen]
pub fn chart_add_line_series(data: &[f64]) -> u32 {
    let pts = parse_line_data(data);
    with_chart_ret(|chart| {
        let id = chart.state.series.add(chart_core::series::Series::line(0, pts));
        chart.state.update_price_scale();
        chart.render();
        id
    })
}

/// Add an area series. Data: flat [time, value, ...]. Returns series ID.
#[wasm_bindgen]
pub fn chart_add_area_series(data: &[f64]) -> u32 {
    let pts = parse_line_data(data);
    with_chart_ret(|chart| {
        let id = chart.state.series.add(chart_core::series::Series::area(0, pts));
        chart.state.update_price_scale();
        chart.render();
        id
    })
}

/// Add a baseline series. Data: flat [time, value, ...]. base_value is the reference line.
#[wasm_bindgen]
pub fn chart_add_baseline_series(data: &[f64], base_value: f64) -> u32 {
    let pts = parse_line_data(data);
    with_chart_ret(|chart| {
        let s = chart_core::series::Series::baseline(0, pts, base_value);
        let id = chart.state.series.add(s);
        chart.state.update_price_scale();
        chart.render();
        id
    })
}

/// Add a histogram series. Data: flat [time, value, ...] (no per-bar colors in WASM for now).
#[wasm_bindgen]
pub fn chart_add_histogram_series(data: &[f64]) -> u32 {
    let pts = parse_histogram_data(data);
    with_chart_ret(|chart| {
        let id = chart.state.series.add(chart_core::series::Series::histogram(0, pts));
        chart.state.update_price_scale();
        chart.render();
        id
    })
}

/// Remove a series by ID.
#[wasm_bindgen]
pub fn chart_remove_series(series_id: u32) {
    with_chart(|chart| {
        chart.state.series.remove(series_id);
        chart.state.update_price_scale();
        true
    });
}

/// Get the number of additional series.
#[wasm_bindgen]
pub fn chart_series_count() -> u32 {
    with_chart_ret(|chart| chart.state.series.len() as u32)
}

// --- Series data management ---

/// Set OHLC data for a specific series. Data: flat [time, open, high, low, close, ...].
#[wasm_bindgen]
pub fn chart_series_set_ohlc_data(series_id: u32, data: &[f64]) {
    let bars = parse_ohlc_data(data);
    with_chart(|chart| {
        if let Some(series) = chart.state.series.get_mut(series_id) {
            series.data = chart_core::series::SeriesData::Ohlc(bars);
            chart.state.update_price_scale();
            return true;
        }
        false
    });
}

/// Set line data for a specific series. Data: flat [time, value, ...].
#[wasm_bindgen]
pub fn chart_series_set_line_data(series_id: u32, data: &[f64]) {
    let pts = parse_line_data(data);
    with_chart(|chart| {
        if let Some(series) = chart.state.series.get_mut(series_id) {
            series.data = chart_core::series::SeriesData::Line(pts);
            chart.state.update_price_scale();
            return true;
        }
        false
    });
}

/// Set histogram data for a specific series. Data: flat [time, value, ...].
#[wasm_bindgen]
pub fn chart_series_set_histogram_data(series_id: u32, data: &[f64]) {
    let pts = parse_histogram_data(data);
    with_chart(|chart| {
        if let Some(series) = chart.state.series.get_mut(series_id) {
            series.data = chart_core::series::SeriesData::Histogram(pts);
            chart.state.update_price_scale();
            return true;
        }
        false
    });
}

/// Update/append a single OHLC bar for a series.
#[wasm_bindgen]
pub fn chart_series_update_ohlc_bar(series_id: u32, time: f64, open: f64, high: f64, low: f64, close: f64) {
    with_chart(|chart| {
        if let Some(series) = chart.state.series.get_mut(series_id) {
            let bar = chart_core::chart_model::OhlcBar {
                time: time as i64, open, high, low, close,
            };
            if let chart_core::series::SeriesData::Ohlc(ref mut bars) = series.data {
                match bars.binary_search_by_key(&bar.time, |b| b.time) {
                    Ok(idx) => bars[idx] = bar,
                    Err(idx) => bars.insert(idx, bar),
                }
                chart.state.update_price_scale();
                return true;
            }
        }
        false
    });
}

/// Update/append a single line point for a series.
#[wasm_bindgen]
pub fn chart_series_update_line_bar(series_id: u32, time: f64, value: f64) {
    with_chart(|chart| {
        if let Some(series) = chart.state.series.get_mut(series_id) {
            let pt = chart_core::series::LineDataPoint {
                time: time as i64, value,
            };
            if let chart_core::series::SeriesData::Line(ref mut pts) = series.data {
                match pts.binary_search_by_key(&pt.time, |p| p.time) {
                    Ok(idx) => pts[idx] = pt,
                    Err(idx) => pts.insert(idx, pt),
                }
                chart.state.update_price_scale();
                return true;
            }
        }
        false
    });
}

/// Pop (remove) the last N data points from a series.
#[wasm_bindgen]
pub fn chart_series_pop(series_id: u32, count: u32) {
    with_chart(|chart| {
        if let Some(series) = chart.state.series.get_mut(series_id) {
            for _ in 0..count {
                match &mut series.data {
                    chart_core::series::SeriesData::Ohlc(bars) => { bars.pop(); }
                    chart_core::series::SeriesData::Line(pts) => { pts.pop(); }
                    chart_core::series::SeriesData::Histogram(pts) => { pts.pop(); }
                }
            }
            chart.state.update_price_scale();
            return true;
        }
        false
    });
}

// --- Options ---

/// Apply chart options from a JSON string.
#[wasm_bindgen]
pub fn chart_apply_options(json: &str) {
    with_chart(|chart| {
        if let Ok(opts) = serde_json::from_str::<chart_core::chart_options::ChartOptions>(json) {
            chart.state.apply_options(opts);
            return true;
        }
        false
    });
}

/// Apply series options from a JSON string.
#[wasm_bindgen]
pub fn chart_series_apply_options(series_id: u32, json: &str) {
    with_chart(|chart| {
        if let Some(series) = chart.state.series.get_mut(series_id) {
            if let Ok(patch) = serde_json::from_str::<serde_json::Value>(json) {
                if let Some(obj) = patch.as_object() {
                    if let Some(c) = obj.get("color").and_then(|v| v.as_str()) {
                        if let Some(rgba) = parse_hex_color(c) {
                            series.line_options.color = rgba;
                        }
                    }
                    if let Some(w) = obj.get("lineWidth").and_then(|v| v.as_f64()) {
                        series.line_options.line_width = w as f32;
                    }
                    if let Some(v) = obj.get("visible").and_then(|v| v.as_bool()) {
                        series.visible = v;
                    }
                    if let Some(bv) = obj.get("baseValue").and_then(|v| v.as_f64()) {
                        series.baseline_options.base_value = bv;
                    }
                }
                return true;
            }
        }
        false
    });
}

// --- Price lines ---

/// Create a price line on a series. Options as JSON. Returns line ID.
#[wasm_bindgen]
pub fn chart_series_create_price_line(series_id: u32, options_json: &str) -> u32 {
    with_chart_ret(|chart| {
        if let Some(series) = chart.state.series.get_mut(series_id) {
            if let Ok(opts) = serde_json::from_str::<chart_core::series::PriceLineOptions>(options_json) {
                let id = series.next_price_line_id;
                series.next_price_line_id += 1;
                series.price_lines.push((id, opts));
                chart.render();
                return id;
            }
        }
        0
    })
}

/// Remove a price line from a series.
#[wasm_bindgen]
pub fn chart_series_remove_price_line(series_id: u32, line_id: u32) {
    with_chart(|chart| {
        if let Some(series) = chart.state.series.get_mut(series_id) {
            series.price_lines.retain(|(id, _)| *id != line_id);
            return true;
        }
        false
    });
}

// --- Multi-pane ---

/// Add a new pane. Returns pane ID.
#[wasm_bindgen]
pub fn chart_add_pane(height_stretch: f32) -> u32 {
    with_chart_ret(|chart| {
        let id = chart.state.add_pane(height_stretch);
        chart.render();
        id
    })
}

/// Remove a pane by ID.
#[wasm_bindgen]
pub fn chart_remove_pane(pane_id: u32) -> bool {
    let mut result = false;
    with_chart(|chart| {
        result = chart.state.remove_pane(pane_id);
        result
    });
    result
}

/// Move a series to a specific pane.
#[wasm_bindgen]
pub fn chart_series_move_to_pane(series_id: u32, pane_id: u32) -> bool {
    let mut result = false;
    with_chart(|chart| {
        result = chart.state.move_series_to_pane(series_id, pane_id);
        result
    });
    result
}

/// Get the number of panes.
#[wasm_bindgen]
pub fn chart_pane_count() -> u32 {
    with_chart_ret(|chart| chart.state.panes.len() as u32)
}

// --- Coordinate translation ---

/// Convert a price value to a Y pixel coordinate.
#[wasm_bindgen]
pub fn chart_price_to_coordinate(price: f64) -> f32 {
    with_chart_ret(|chart| {
        chart.state.panes[0]
            .price_scale
            .price_to_y(price, &chart.state.panes[0].layout_rect)
    })
}

/// Convert a Y pixel coordinate to a price value.
#[wasm_bindgen]
pub fn chart_coordinate_to_price(y: f32) -> f64 {
    with_chart_ret(|chart| {
        chart.state.panes[0]
            .price_scale
            .y_to_price(y, &chart.state.panes[0].layout_rect)
    })
}

/// Convert a timestamp to an X pixel coordinate.
#[wasm_bindgen]
pub fn chart_time_to_coordinate(time: f64) -> f32 {
    with_chart_ret(|chart| {
        let time_i64 = time as i64;
        let idx = chart.state.data.bars.binary_search_by_key(&time_i64, |b| b.time);
        match idx {
            Ok(i) => chart.state.time_scale.index_to_x(i, &chart.state.layout.plot_area),
            Err(i) => chart.state.time_scale.index_to_x(i, &chart.state.layout.plot_area),
        }
    })
}

// --- Crosshair control ---

/// Programmatically set crosshair position.
#[wasm_bindgen]
pub fn chart_set_crosshair_position(price: f64, time: f64, series_id: u32) {
    with_chart(|chart| {
        chart.state.set_crosshair_position(price, time as i64, series_id)
    });
}

/// Clear the crosshair.
#[wasm_bindgen]
pub fn chart_clear_crosshair_position() {
    with_chart(|chart| chart.state.clear_crosshair_position());
}

// --- Read APIs ---

/// Get the number of data points in a series.
#[wasm_bindgen]
pub fn chart_series_data_length(series_id: u32) -> u32 {
    with_chart_ret(|chart| {
        if let Some(series) = chart.state.series.get(series_id) {
            match &series.data {
                chart_core::series::SeriesData::Ohlc(bars) => bars.len() as u32,
                chart_core::series::SeriesData::Line(pts) => pts.len() as u32,
                chart_core::series::SeriesData::Histogram(pts) => pts.len() as u32,
            }
        } else {
            0
        }
    })
}

/// Resize the chart.
#[wasm_bindgen]
pub fn chart_resize(width: u32, height: u32, scale_factor: f64) {
    with_chart(|chart| {
        chart.state.resize(width as f32, height as f32, scale_factor);
        true
    });
}

/// Pinch zoom (two-finger).
#[wasm_bindgen]
pub fn chart_pinch(scale: f32, center_x: f32, center_y: f32) {
    with_chart(|chart| chart.state.pinch(scale, center_x, center_y));
}

// --- Data parsing helpers ---

fn parse_ohlc_data(data: &[f64]) -> Vec<chart_core::chart_model::OhlcBar> {
    let count = data.len() / 5;
    (0..count)
        .map(|i| {
            let base = i * 5;
            chart_core::chart_model::OhlcBar {
                time: data[base] as i64,
                open: data[base + 1],
                high: data[base + 2],
                low: data[base + 3],
                close: data[base + 4],
            }
        })
        .collect()
}

fn parse_line_data(data: &[f64]) -> Vec<chart_core::series::LineDataPoint> {
    let count = data.len() / 2;
    (0..count)
        .map(|i| {
            let base = i * 2;
            chart_core::series::LineDataPoint {
                time: data[base] as i64,
                value: data[base + 1],
            }
        })
        .collect()
}

fn parse_histogram_data(data: &[f64]) -> Vec<chart_core::series::HistogramDataPoint> {
    let count = data.len() / 2;
    (0..count)
        .map(|i| {
            let base = i * 2;
            chart_core::series::HistogramDataPoint {
                time: data[base] as i64,
                value: data[base + 1],
                color: None,
            }
        })
        .collect()
}

/// Parse a CSS hex color string (#RGB, #RRGGBB, #RRGGBBAA) into [f32; 4] RGBA.
fn parse_hex_color(s: &str) -> Option<[f32; 4]> {
    let s = s.trim_start_matches('#');
    let (r, g, b, a) = match s.len() {
        3 => {
            let r = u8::from_str_radix(&s[0..1], 16).ok()? * 17;
            let g = u8::from_str_radix(&s[1..2], 16).ok()? * 17;
            let b = u8::from_str_radix(&s[2..3], 16).ok()? * 17;
            (r, g, b, 255u8)
        }
        6 => {
            let r = u8::from_str_radix(&s[0..2], 16).ok()?;
            let g = u8::from_str_radix(&s[2..4], 16).ok()?;
            let b = u8::from_str_radix(&s[4..6], 16).ok()?;
            (r, g, b, 255u8)
        }
        8 => {
            let r = u8::from_str_radix(&s[0..2], 16).ok()?;
            let g = u8::from_str_radix(&s[2..4], 16).ok()?;
            let b = u8::from_str_radix(&s[4..6], 16).ok()?;
            let a = u8::from_str_radix(&s[6..8], 16).ok()?;
            (r, g, b, a)
        }
        _ => return None,
    };
    Some([r as f32 / 255.0, g as f32 / 255.0, b as f32 / 255.0, a as f32 / 255.0])
}
