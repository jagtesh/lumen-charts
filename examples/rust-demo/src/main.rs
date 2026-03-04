//! Lumen Charts — Rust Demo
//!
//! Cross-platform demo using winit + wgpu + egui.
//! Showcases chart type switching, overlay, and MACD indicator.
//!
//! **Migrated to lumen-charts-sdk** — all chart interactions use the safe v5 API.
//! No unsafe blocks needed for chart operations.
//!
//! Keyboard shortcuts (in addition to egui toolbar):
//!   1-6  Switch chart type (OHLC, Candle, Line, Area, Hist, Baseline)
//!   F    Fit content
//!   O    Toggle overlay
//!   M    Toggle MACD

use lumen_charts::renderers::VelloRenderer;
use lumen_charts::sample_data::sample_data;
use lumen_charts_sdk::{
    ChartApi, Color, HistogramDataPoint, LineDataPoint, OhlcBar, PaneApi, SeriesApi,
    SeriesDefinition,
};
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
            Some(Color([0.16, 0.76, 0.49, 0.8f32]))
        } else {
            Some(Color([0.94, 0.27, 0.27, 0.8f32]))
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
    chart: ChartApi,
    // egui integration
    egui_ctx: egui::Context,
    egui_state: egui_winit::State,
    egui_renderer: egui_wgpu::Renderer,
    // Chart state
    current_series_type: usize,
    overlay_active: bool,
    overlay_series: Option<SeriesApi>,
    macd_active: bool,
    macd_pane: Option<PaneApi>,
    macd_series: Vec<SeriesApi>,
    // Cached sample data
    sample_bars: Vec<OhlcBar>,
    // Mouse tracking
    cursor_pos: (f32, f32),
    scale_factor: f64,
}

impl AppState {
    fn toggle_overlay(&mut self) {
        if let Some(series) = self.overlay_series.take() {
            // Remove the overlay series — safe, no unsafe needed
            self.chart.remove_series(&series);
            self.chart.render();
            self.overlay_active = false;
        } else {
            // Create an area overlay offset below the main price
            let overlay_data: Vec<LineDataPoint> = self
                .sample_bars
                .iter()
                .map(|b| LineDataPoint {
                    time: b.time,
                    value: b.close - 15.0,
                })
                .collect();

            // v5 unified addSeries API — no unsafe!
            let series = self.chart.add_series(SeriesDefinition::Area);
            series.set_line_data(&mut self.chart, &overlay_data);
            self.overlay_series = Some(series);
            self.chart.render();
            self.overlay_active = true;
        }
    }

    fn toggle_macd(&mut self) {
        if let Some(pane) = self.macd_pane.take() {
            // Remove all MACD series and the pane — safe SDK calls
            for series in &self.macd_series {
                self.chart.remove_series(series);
            }
            self.chart.remove_pane(&pane);
            self.macd_series.clear();
            self.chart.render();
            self.macd_active = false;
        } else {
            let macd = calculate_macd(&self.sample_bars);

            // Add a sub-pane for MACD (30% height)
            let pane = self.chart.add_pane(0.3);

            // Histogram series
            let hist_series = self.chart.add_series(SeriesDefinition::Histogram);
            hist_series.set_histogram_data(&mut self.chart, &macd.histogram);
            hist_series.move_to_pane(&mut self.chart, &pane);

            // MACD line (blue)
            let macd_line_series = self.chart.add_series(SeriesDefinition::Line);
            macd_line_series.set_line_data(&mut self.chart, &macd.macd_line);
            macd_line_series.move_to_pane(&mut self.chart, &pane);
            macd_line_series.apply_options(
                &mut self.chart,
                r#"{"color":[0.2,0.6,1.0,1.0],"lineWidth":1.5}"#,
            );

            // Signal line (orange)
            let signal_series = self.chart.add_series(SeriesDefinition::Line);
            signal_series.set_line_data(&mut self.chart, &macd.signal_line);
            signal_series.move_to_pane(&mut self.chart, &pane);
            signal_series.apply_options(
                &mut self.chart,
                r#"{"color":[1.0,0.6,0.2,1.0],"lineWidth":1.5}"#,
            );

            self.macd_pane = Some(pane);
            self.macd_series = vec![hist_series, macd_line_series, signal_series];
            self.chart.render();
            self.macd_active = true;
        }
    }

    fn set_series_type(&mut self, type_idx: usize) {
        self.current_series_type = type_idx;
        self.chart.set_series_type(type_idx as u32);
        self.chart.render();
    }

    fn fit_content(&mut self) {
        self.chart.fit_content();
        self.chart.render();
    }

    fn render_egui_and_chart(&mut self) {
        // --- Collect actions ---
        // egui closure borrows fields of self, so we can't call &mut self methods
        // inside it. Instead, collect flags and execute after the closure.
        let mut action_set_type: Option<usize> = None;
        let mut action_fit = false;
        let mut action_overlay = false;
        let mut action_macd = false;

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
                            action_set_type = Some(i);
                        }
                    }
                    ui.separator();
                    if ui.button("Fit Content").clicked() {
                        action_fit = true;
                    }
                    if ui
                        .selectable_label(self.overlay_active, "Overlay")
                        .clicked()
                    {
                        action_overlay = true;
                    }
                    if ui.selectable_label(self.macd_active, "MACD").clicked() {
                        action_macd = true;
                    }
                    ui.separator();
                    ui.label(format!(
                        "{}  •  {} bars",
                        SERIES_TYPES[self.current_series_type],
                        self.sample_bars.len()
                    ));
                });
            });
        });

        // --- Execute deferred actions ---
        if let Some(idx) = action_set_type {
            self.set_series_type(idx);
        }
        if action_fit {
            self.fit_content();
        }
        if action_overlay {
            self.toggle_overlay();
        }
        if action_macd {
            self.toggle_macd();
        }

        self.egui_state
            .handle_platform_output(&self.window, full_output.platform_output);

        // --- Downcast renderer to VelloRenderer for wgpu access ---
        // We access the inner Chart directly to split-borrow renderer vs state.
        let inner = &mut self.chart.inner;
        let pipeline = inner
            .renderer
            .as_any_mut()
            .downcast_mut::<VelloRenderer>()
            .expect("Rust demo requires VelloRenderer renderer");

        // --- Single surface acquire ---
        let surface_texture = match pipeline.surface.get_current_texture() {
            Ok(t) => t,
            Err(_) => return,
        };

        // --- Render chart via Vello (bypasses chart_render to avoid double-acquire) ---
        {
            use lumen_charts::backends::VelloBackend;
            use lumen_charts::chart_renderer::{render_bottom_scene, render_crosshair_scene};

            pipeline.backend.reset();

            let mut bottom_backend = VelloBackend::new();
            render_bottom_scene(&mut bottom_backend, &inner.state);
            let bottom_scene = bottom_backend.scene;
            pipeline.backend.scene_mut().append(&bottom_scene, None);
            pipeline.cached_bottom_scene = Some(bottom_scene);

            render_crosshair_scene(&mut pipeline.backend, &inner.state);

            let render_params = vello::RenderParams {
                base_color: vello::peniko::Color::BLACK,
                width: pipeline.surface_config.width,
                height: pipeline.surface_config.height,
                antialiasing_method: vello::AaConfig::Area,
            };

            pipeline
                .vello_renderer
                .render_to_surface(
                    &pipeline.device,
                    &pipeline.queue,
                    pipeline.backend.scene(),
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
                    pipeline.surface_config.width,
                    pipeline.surface_config.height,
                ],
                pixels_per_point: self.scale_factor as f32,
            };

            let clipped_primitives = self
                .egui_ctx
                .tessellate(full_output.shapes, full_output.pixels_per_point);

            for (id, image_delta) in &full_output.textures_delta.set {
                self.egui_renderer.update_texture(
                    &pipeline.device,
                    &pipeline.queue,
                    *id,
                    image_delta,
                );
            }

            let mut encoder =
                pipeline
                    .device
                    .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                        label: Some("egui-encoder"),
                    });

            self.egui_renderer.update_buffers(
                &pipeline.device,
                &pipeline.queue,
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

            pipeline.queue.submit(std::iter::once(encoder.finish()));

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

        // Create VelloRenderer and Chart via SDK
        let logical_w = (size.width as f64 / scale_factor) as u32;
        let logical_h = (size.height as f64 / scale_factor) as u32;
        let pipeline = VelloRenderer::new(instance, surface, logical_w, logical_h, scale_factor);

        // Need device/format before moving pipeline into Chart
        let surface_format = pipeline.surface_config.format;
        let device_ref = &pipeline.device;

        // Create egui renderer before pipeline moves into Chart
        let egui_ctx = egui::Context::default();
        egui_ctx.set_visuals(egui::Visuals::dark());
        let egui_state = egui_winit::State::new(
            egui_ctx.clone(),
            egui::ViewportId::ROOT,
            event_loop,
            Some(scale_factor as f32),
            None,
            None,
        );
        let egui_renderer = egui_wgpu::Renderer::new(device_ref, surface_format, None, 1, false);

        // Create ChartApi via SDK — wraps Chart with safe v5 methods
        let mut chart =
            ChartApi::with_renderer(Box::new(pipeline), logical_w, logical_h, scale_factor);

        // Load sample data using safe SDK
        let bars = sample_data();
        chart.set_data(bars.clone());
        chart.fit_content();
        chart.render();

        self.state = Some(AppState {
            window,
            chart,
            egui_ctx,
            egui_state,
            egui_renderer,
            current_series_type: 0,
            overlay_active: false,
            overlay_series: None,
            macd_active: false,
            macd_pane: None,
            macd_series: Vec::new(),
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
                    state.chart.resize(logical_w, logical_h, state.scale_factor);
                    state.window.request_redraw();
                }
            }
            WindowEvent::ScaleFactorChanged { scale_factor, .. } => {
                state.scale_factor = scale_factor;
            }
            WindowEvent::RedrawRequested => {
                state.render_egui_and_chart();
            }
            WindowEvent::CursorMoved { position, .. } => {
                if !egui_wants_input {
                    let x = (position.x / state.scale_factor) as f32;
                    let y = (position.y / state.scale_factor) as f32;
                    state.cursor_pos = (x, y);
                    if state.chart.pointer_move(x, y) {
                        state.window.request_redraw();
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
                    let redraw = match btn_state {
                        ElementState::Pressed => state.chart.pointer_down(x, y, 0),
                        ElementState::Released => state.chart.pointer_up(x, y, 0),
                    };
                    if redraw {
                        state.window.request_redraw();
                    }
                }
            }
            WindowEvent::MouseWheel { delta, .. } => {
                if !egui_wants_input {
                    let (dx, dy) = match delta {
                        MouseScrollDelta::LineDelta(x, y) => (x * 20.0, y * 20.0),
                        MouseScrollDelta::PixelDelta(pos) => (pos.x as f32, pos.y as f32),
                    };
                    // Scroll horizontally to pan, vertically to zoom
                    // (macOS trackpad convention: vertical scroll = zoom)
                    let mut redraw = false;
                    if dx.abs() > 0.1 {
                        redraw |= state.chart.scroll(-dx, 0.0);
                    }
                    if dy.abs() > 0.1 {
                        let factor = 1.0 - dy * 0.003;
                        let (cx, _) = state.cursor_pos;
                        redraw |= state.chart.zoom(factor, cx);
                    }
                    if redraw {
                        state.window.request_redraw();
                    }
                }
            }
            // Trackpad pinch-to-zoom (macOS)
            WindowEvent::PinchGesture { delta, .. } => {
                if !egui_wants_input {
                    let factor = 1.0 + delta as f32;
                    let (cx, cy) = state.cursor_pos;
                    if state.chart.pinch(factor, cx, cy) {
                        state.window.request_redraw();
                    }
                }
            }
            WindowEvent::CursorLeft { .. } => {
                if !egui_wants_input {
                    if state.chart.pointer_leave() {
                        state.window.request_redraw();
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
                                    if state.chart.key_down(*chart_code) {
                                        state.window.request_redraw();
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
