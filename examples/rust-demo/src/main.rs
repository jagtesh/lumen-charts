//! Lumen Charts — Rust Demo
//!
//! Cross-platform demo using winit + wgpu + egui.
//! Showcases chart type switching, overlay, and MACD indicator.
//!
//! Keyboard shortcuts (in addition to egui toolbar):
//!   1-6  Switch chart type (OHLC, Candle, Line, Area, Hist, Baseline)
//!   F    Fit content
//!   O    Toggle overlay
//!   M    Toggle MACD
#![allow(unused_unsafe)]

use lumen_charts::chart_model::OhlcBar;
use lumen_charts::sample_data::sample_data;
use lumen_charts::series::{HistogramDataPoint, LineDataPoint};
use lumen_charts::Chart;
use std::sync::Arc;
use winit::application::ApplicationHandler;
use winit::event::{ElementState, MouseButton, MouseScrollDelta, WindowEvent};
use winit::event_loop::{ActiveEventLoop, EventLoop};
use winit::keyboard::{KeyCode, PhysicalKey};
use winit::window::{Window, WindowAttributes, WindowId};

// ---------------------------------------------------------------------------
// MACD calculation
// ---------------------------------------------------------------------------

fn ema(values: &[f64], period: usize) -> Vec<f64> {
    if values.len() < period {
        return values.to_vec();
    }
    let k = 2.0 / (period as f64 + 1.0);
    let mut result = vec![0.0f64; values.len()];
    let sma: f64 = values[..period].iter().sum::<f64>() / period as f64;
    result[period - 1] = sma;
    for i in period..values.len() {
        result[i] = values[i] * k + result[i - 1] * (1.0 - k);
    }
    result
}

struct MacdData {
    macd_line: Vec<LineDataPoint>,
    signal_line: Vec<LineDataPoint>,
    histogram: Vec<HistogramDataPoint>,
}

fn calculate_macd(bars: &[OhlcBar]) -> MacdData {
    let closes: Vec<f64> = bars.iter().map(|b| b.close).collect();
    let ema12 = ema(&closes, 12);
    let ema26 = ema(&closes, 26);

    let start_idx = 25;
    let mut macd_values = Vec::new();
    let mut macd_times = Vec::new();
    for i in start_idx..bars.len() {
        macd_values.push(ema12[i] - ema26[i]);
        macd_times.push(bars[i].time);
    }

    let signal_values = ema(&macd_values, 9);
    let signal_start = 8;

    let mut macd_line = Vec::new();
    let mut signal_line = Vec::new();
    let mut histogram = Vec::new();

    for i in signal_start..macd_values.len() {
        let time = macd_times[i];
        let macd = macd_values[i];
        let signal = signal_values[i];
        let hist = macd - signal;

        macd_line.push(LineDataPoint { time, value: macd });
        signal_line.push(LineDataPoint {
            time,
            value: signal,
        });

        let color = if hist >= 0.0 {
            Some([0.16, 0.76, 0.49, 0.8f32])
        } else {
            Some([0.94, 0.27, 0.27, 0.8f32])
        };
        histogram.push(HistogramDataPoint {
            time,
            value: hist,
            color,
        });
    }

    MacdData {
        macd_line,
        signal_line,
        histogram,
    }
}

// ---------------------------------------------------------------------------
// App state
// ---------------------------------------------------------------------------

const SERIES_TYPES: [&str; 6] = [
    "OHLC",
    "Candlestick",
    "Line",
    "Area",
    "Histogram",
    "Baseline",
];

struct AppState {
    window: Arc<Window>,
    chart: Chart,
    // egui integration
    egui_ctx: egui::Context,
    egui_state: egui_winit::State,
    egui_renderer: egui_wgpu::Renderer,
    // Chart state
    current_series_type: usize,
    overlay_active: bool,
    overlay_series_id: Option<u32>,
    macd_active: bool,
    macd_pane_id: Option<u32>,
    macd_series_ids: Vec<u32>,
    // Cached sample data
    sample_bars: Vec<OhlcBar>,
    // Mouse tracking
    cursor_pos: (f32, f32),
    scale_factor: f64,
}

impl AppState {
    fn toggle_overlay(&mut self) {
        if let Some(id) = self.overlay_series_id.take() {
            unsafe {
                let ptr = &mut self.chart as *mut Chart;
                lumen_charts::chart_remove_series(ptr, id);
                lumen_charts::chart_render(ptr);
            }
            self.overlay_active = false;
        } else {
            let overlay_data: Vec<LineDataPoint> = self
                .sample_bars
                .iter()
                .map(|b| LineDataPoint {
                    time: b.time,
                    value: b.close - 15.0,
                })
                .collect();
            let times: Vec<i64> = overlay_data.iter().map(|d| d.time).collect();
            let values: Vec<f64> = overlay_data.iter().map(|d| d.value).collect();
            unsafe {
                let ptr = &mut self.chart as *mut Chart;
                let id = lumen_charts::chart_add_area_series(
                    ptr,
                    times.as_ptr(),
                    values.as_ptr(),
                    overlay_data.len() as u32,
                );
                self.overlay_series_id = Some(id);
                lumen_charts::chart_render(ptr);
            }
            self.overlay_active = true;
        }
    }

    fn toggle_macd(&mut self) {
        if let Some(pane_id) = self.macd_pane_id.take() {
            unsafe {
                let ptr = &mut self.chart as *mut Chart;
                for &id in &self.macd_series_ids {
                    lumen_charts::chart_remove_series(ptr, id);
                }
                lumen_charts::chart_remove_pane(ptr, pane_id);
                lumen_charts::chart_render(ptr);
            }
            self.macd_series_ids.clear();
            self.macd_active = false;
        } else {
            let macd = calculate_macd(&self.sample_bars);
            unsafe {
                let ptr = &mut self.chart as *mut Chart;
                let pane_id = lumen_charts::chart_add_pane(ptr, 0.3);
                self.macd_pane_id = Some(pane_id);

                // Histogram
                let h_times: Vec<i64> = macd.histogram.iter().map(|d| d.time).collect();
                let h_values: Vec<f64> = macd.histogram.iter().map(|d| d.value).collect();
                let h_colors: Vec<u32> = macd
                    .histogram
                    .iter()
                    .map(|d| {
                        let c = d.color.unwrap_or([0.5, 0.5, 0.5, 1.0]);
                        let r = (c[0] * 255.0) as u32;
                        let g = (c[1] * 255.0) as u32;
                        let b = (c[2] * 255.0) as u32;
                        let a = (c[3] * 255.0) as u32;
                        (r << 24) | (g << 16) | (b << 8) | a
                    })
                    .collect();
                let hist_id = lumen_charts::chart_add_histogram_series(
                    ptr,
                    h_times.as_ptr(),
                    h_values.as_ptr(),
                    h_colors.as_ptr(),
                    macd.histogram.len() as u32,
                );
                lumen_charts::chart_series_move_to_pane(ptr, hist_id, pane_id);

                // MACD line (blue)
                let m_times: Vec<i64> = macd.macd_line.iter().map(|d| d.time).collect();
                let m_values: Vec<f64> = macd.macd_line.iter().map(|d| d.value).collect();
                let macd_line_id = lumen_charts::chart_add_line_series(
                    ptr,
                    m_times.as_ptr(),
                    m_values.as_ptr(),
                    macd.macd_line.len() as u32,
                );
                lumen_charts::chart_series_move_to_pane(ptr, macd_line_id, pane_id);
                // Apply blue color
                let opts = r#"{"color":[0.2,0.6,1.0,1.0],"lineWidth":1.5}"#;
                let c_str = std::ffi::CString::new(opts).unwrap();
                lumen_charts::chart_series_apply_options(ptr, macd_line_id, c_str.as_ptr());

                // Signal line (orange)
                let s_times: Vec<i64> = macd.signal_line.iter().map(|d| d.time).collect();
                let s_values: Vec<f64> = macd.signal_line.iter().map(|d| d.value).collect();
                let signal_id = lumen_charts::chart_add_line_series(
                    ptr,
                    s_times.as_ptr(),
                    s_values.as_ptr(),
                    macd.signal_line.len() as u32,
                );
                lumen_charts::chart_series_move_to_pane(ptr, signal_id, pane_id);
                let opts = r#"{"color":[1.0,0.6,0.2,1.0],"lineWidth":1.5}"#;
                let c_str = std::ffi::CString::new(opts).unwrap();
                lumen_charts::chart_series_apply_options(ptr, signal_id, c_str.as_ptr());

                self.macd_series_ids = vec![hist_id, macd_line_id, signal_id];
                lumen_charts::chart_render(ptr);
            }
            self.macd_active = true;
        }
    }

    fn set_series_type(&mut self, type_idx: usize) {
        self.current_series_type = type_idx;
        unsafe {
            let ptr = &mut self.chart as *mut Chart;
            lumen_charts::chart_set_series_type(ptr, type_idx as u32);
            lumen_charts::chart_render(ptr);
        }
    }

    fn fit_content(&mut self) {
        unsafe {
            let ptr = &mut self.chart as *mut Chart;
            lumen_charts::chart_fit_content(ptr);
            lumen_charts::chart_render(ptr);
        }
    }

    fn render_egui_and_chart(&mut self) {
        // --- egui input + UI layout ---
        let raw_input = self.egui_state.take_egui_input(&self.window);
        let full_output = self.egui_ctx.run(raw_input, |ctx| {
            egui::TopBottomPanel::top("toolbar").show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.label("Chart Type:");
                    for (i, name) in SERIES_TYPES.iter().enumerate() {
                        if ui
                            .selectable_label(self.current_series_type == i, *name)
                            .clicked()
                        {
                            self.current_series_type = i;
                        }
                    }
                    ui.separator();
                    if ui.button("Fit Content").clicked() {}
                    if ui
                        .selectable_label(self.overlay_active, "Overlay")
                        .clicked()
                    {}
                    if ui.selectable_label(self.macd_active, "MACD").clicked() {}
                    ui.separator();
                    ui.label(format!(
                        "{}  •  {} bars",
                        SERIES_TYPES[self.current_series_type],
                        self.sample_bars.len()
                    ));
                });
            });
        });

        self.egui_state
            .handle_platform_output(&self.window, full_output.platform_output);

        // --- Single surface acquire ---
        let surface_texture = match self.chart.surface.get_current_texture() {
            Ok(t) => t,
            Err(_) => return,
        };

        // --- Render chart via Vello (bypasses chart_render to avoid double-acquire) ---
        {
            use lumen_charts::backend_vello::VelloBackend;
            use lumen_charts::chart_renderer::{render_bottom_scene, render_crosshair_scene};

            self.chart.backend.reset();

            let mut bottom_backend = VelloBackend::new();
            render_bottom_scene(&mut bottom_backend, &self.chart.state);
            let bottom_scene = bottom_backend.scene;
            self.chart.backend.scene_mut().append(&bottom_scene, None);
            self.chart.cached_bottom_scene = Some(bottom_scene);

            render_crosshair_scene(&mut self.chart.backend, &self.chart.state);

            let render_params = vello::RenderParams {
                base_color: vello::peniko::Color::BLACK,
                width: self.chart.surface_config.width,
                height: self.chart.surface_config.height,
                antialiasing_method: vello::AaConfig::Area,
            };

            self.chart
                .vello_renderer
                .render_to_surface(
                    &self.chart.device,
                    &self.chart.queue,
                    self.chart.backend.scene(),
                    &surface_texture,
                    &render_params,
                )
                .expect("Vello render failed");
        }

        // --- Overlay egui on top ---
        {
            let surface_view = surface_texture
                .texture
                .create_view(&wgpu::TextureViewDescriptor::default());

            let screen_descriptor = egui_wgpu::ScreenDescriptor {
                size_in_pixels: [
                    self.chart.surface_config.width,
                    self.chart.surface_config.height,
                ],
                pixels_per_point: self.scale_factor as f32,
            };

            let clipped_primitives = self
                .egui_ctx
                .tessellate(full_output.shapes, full_output.pixels_per_point);

            for (id, image_delta) in &full_output.textures_delta.set {
                self.egui_renderer.update_texture(
                    &self.chart.device,
                    &self.chart.queue,
                    *id,
                    image_delta,
                );
            }

            let mut encoder =
                self.chart
                    .device
                    .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                        label: Some("egui-encoder"),
                    });

            self.egui_renderer.update_buffers(
                &self.chart.device,
                &self.chart.queue,
                &mut encoder,
                &clipped_primitives,
                &screen_descriptor,
            );

            {
                let render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                    label: Some("egui-render-pass"),
                    color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                        view: &surface_view,
                        resolve_target: None,
                        ops: wgpu::Operations {
                            load: wgpu::LoadOp::Load,
                            store: wgpu::StoreOp::Store,
                        },
                    })],
                    ..Default::default()
                });
                self.egui_renderer.render(
                    &mut render_pass.forget_lifetime(),
                    &clipped_primitives,
                    &screen_descriptor,
                );
            }

            self.chart.queue.submit(std::iter::once(encoder.finish()));

            for id in &full_output.textures_delta.free {
                self.egui_renderer.free_texture(id);
            }
        }

        // --- Single present ---
        surface_texture.present();
    }
}

// ---------------------------------------------------------------------------
// Application handler
// ---------------------------------------------------------------------------

struct App {
    state: Option<AppState>,
}

impl ApplicationHandler for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.state.is_some() {
            return;
        }

        let window_attrs = WindowAttributes::default()
            .with_title("Lumen Charts — Rust Demo")
            .with_inner_size(winit::dpi::LogicalSize::new(1000u32, 732u32))
            .with_min_inner_size(winit::dpi::LogicalSize::new(600u32, 400u32));

        let window = Arc::new(event_loop.create_window(window_attrs).unwrap());
        let scale_factor = window.scale_factor();
        let size = window.inner_size();

        // Create wgpu surface from the window
        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
            backends: wgpu::Backends::all(),
            ..Default::default()
        });
        let surface = instance.create_surface(window.clone()).unwrap();

        // Create chart from the instance + surface
        let logical_w = (size.width as f64 / scale_factor) as u32;
        let logical_h = (size.height as f64 / scale_factor) as u32;
        let mut chart =
            Chart::new_from_surface(instance, surface, logical_w, logical_h, scale_factor);

        // Load sample data
        let bars = sample_data();
        let flat: Vec<f64> = bars
            .iter()
            .flat_map(|b| vec![b.time as f64, b.open, b.high, b.low, b.close])
            .collect();
        unsafe {
            let ptr = &mut chart as *mut Chart;
            lumen_charts::chart_set_data(ptr, flat.as_ptr(), bars.len() as u32);
            lumen_charts::chart_fit_content(ptr);
            lumen_charts::chart_render(ptr);
        }

        // Set up egui
        let egui_ctx = egui::Context::default();

        // Style: dark mode
        egui_ctx.set_visuals(egui::Visuals::dark());

        let egui_state = egui_winit::State::new(
            egui_ctx.clone(),
            egui::ViewportId::ROOT,
            event_loop,
            Some(scale_factor as f32),
            None,
            None,
        );

        let egui_renderer =
            egui_wgpu::Renderer::new(&chart.device, chart.surface_config.format, None, 1, false);

        self.state = Some(AppState {
            window,
            chart,
            egui_ctx,
            egui_state,
            egui_renderer,
            current_series_type: 0,
            overlay_active: false,
            overlay_series_id: None,
            macd_active: false,
            macd_pane_id: None,
            macd_series_ids: Vec::new(),
            sample_bars: bars,
            cursor_pos: (0.0, 0.0),
            scale_factor,
        });
    }

    fn window_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        _window_id: WindowId,
        event: WindowEvent,
    ) {
        let state = match &mut self.state {
            Some(s) => s,
            None => return,
        };

        // Let egui process the event first
        let response = state.egui_state.on_window_event(&state.window, &event);
        let egui_wants_input = response.consumed;

        match event {
            WindowEvent::CloseRequested => event_loop.exit(),
            WindowEvent::Resized(size) => {
                if size.width > 0 && size.height > 0 {
                    let logical_w = (size.width as f64 / state.scale_factor) as u32;
                    let logical_h = (size.height as f64 / state.scale_factor) as u32;
                    unsafe {
                        let ptr = &mut state.chart as *mut Chart;
                        lumen_charts::chart_resize(ptr, logical_w, logical_h, state.scale_factor);
                    }
                    state.window.request_redraw();
                }
            }
            WindowEvent::ScaleFactorChanged { scale_factor, .. } => {
                state.scale_factor = scale_factor;
            }
            WindowEvent::RedrawRequested => {
                // Render chart, then egui overlay
                state.render_egui_and_chart();
            }
            WindowEvent::CursorMoved { position, .. } => {
                if !egui_wants_input {
                    let x = (position.x / state.scale_factor) as f32;
                    let y = (position.y / state.scale_factor) as f32;
                    state.cursor_pos = (x, y);
                    unsafe {
                        let ptr = &mut state.chart as *mut Chart;
                        if lumen_charts::chart_pointer_move(ptr, x, y) {
                            state.window.request_redraw();
                        }
                    }
                }
            }
            WindowEvent::MouseInput {
                state: btn_state,
                button: MouseButton::Left,
                ..
            } => {
                if !egui_wants_input {
                    let (x, y) = state.cursor_pos;
                    unsafe {
                        let ptr = &mut state.chart as *mut Chart;
                        let redraw = match btn_state {
                            ElementState::Pressed => lumen_charts::chart_pointer_down(ptr, x, y, 0),
                            ElementState::Released => lumen_charts::chart_pointer_up(ptr, x, y, 0),
                        };
                        if redraw {
                            state.window.request_redraw();
                        }
                    }
                }
            }
            WindowEvent::MouseWheel { delta, .. } => {
                if !egui_wants_input {
                    let (dx, dy) = match delta {
                        MouseScrollDelta::LineDelta(x, y) => (x * 20.0, y * 20.0),
                        MouseScrollDelta::PixelDelta(pos) => (pos.x as f32, pos.y as f32),
                    };
                    unsafe {
                        let ptr = &mut state.chart as *mut Chart;
                        if lumen_charts::chart_scroll(ptr, -dx, dy) {
                            state.window.request_redraw();
                        }
                    }
                }
            }
            WindowEvent::CursorLeft { .. } => {
                if !egui_wants_input {
                    unsafe {
                        let ptr = &mut state.chart as *mut Chart;
                        if lumen_charts::chart_pointer_leave(ptr) {
                            state.window.request_redraw();
                        }
                    }
                }
            }
            WindowEvent::KeyboardInput { event, .. } => {
                if event.state == ElementState::Pressed && !egui_wants_input {
                    match event.physical_key {
                        PhysicalKey::Code(KeyCode::Digit1) => state.set_series_type(0),
                        PhysicalKey::Code(KeyCode::Digit2) => state.set_series_type(1),
                        PhysicalKey::Code(KeyCode::Digit3) => state.set_series_type(2),
                        PhysicalKey::Code(KeyCode::Digit4) => state.set_series_type(3),
                        PhysicalKey::Code(KeyCode::Digit5) => state.set_series_type(4),
                        PhysicalKey::Code(KeyCode::Digit6) => state.set_series_type(5),
                        PhysicalKey::Code(KeyCode::KeyF) => state.fit_content(),
                        PhysicalKey::Code(KeyCode::KeyO) => state.toggle_overlay(),
                        PhysicalKey::Code(KeyCode::KeyM) => state.toggle_macd(),
                        _ => {
                            // Map arrow keys etc to chart key_down
                            let key_map: &[(KeyCode, u32)] = &[
                                (KeyCode::ArrowLeft, 37),
                                (KeyCode::ArrowRight, 39),
                                (KeyCode::ArrowUp, 38),
                                (KeyCode::ArrowDown, 40),
                                (KeyCode::Equal, 187),
                                (KeyCode::Minus, 189),
                                (KeyCode::Home, 36),
                                (KeyCode::End, 35),
                            ];
                            if let PhysicalKey::Code(code) = event.physical_key {
                                if let Some((_, chart_code)) =
                                    key_map.iter().find(|(k, _)| *k == code)
                                {
                                    unsafe {
                                        let ptr = &mut state.chart as *mut Chart;
                                        if lumen_charts::chart_key_down(ptr, *chart_code) {
                                            state.window.request_redraw();
                                        }
                                    }
                                }
                            }
                        }
                    }
                    state.window.request_redraw();
                }
            }
            _ => {}
        }
    }
}

// ---------------------------------------------------------------------------
// Main
// ---------------------------------------------------------------------------

fn main() {
    env_logger::init();
    let event_loop = EventLoop::new().unwrap();
    event_loop.set_control_flow(winit::event_loop::ControlFlow::Wait);
    let mut app = App { state: None };
    event_loop.run_app(&mut app).unwrap();
}
