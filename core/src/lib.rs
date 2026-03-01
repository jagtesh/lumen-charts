pub mod chart_model;
pub mod chart_options;
pub mod chart_renderer;
pub mod chart_state;
pub mod data_layer;
pub mod overlays;
pub mod price_scale;
pub mod sample_data;
pub mod series;
pub mod text_render;
pub mod tick_marks;
pub mod time_scale;

// ---------------------------------------------------------------------------
// Native C-ABI (macOS/iOS/Linux/Windows) — excluded from WASM builds
// ---------------------------------------------------------------------------
#[cfg(not(target_arch = "wasm32"))]
mod native {
    use std::ffi::c_void;

    use crate::chart_model::ChartData;
    use crate::chart_renderer::render_chart;
    use crate::chart_state::ChartState;

    use vello::wgpu;
    use vello::{AaConfig, Renderer as VelloRenderer, RendererOptions, Scene};

    #[repr(C)]
    pub struct ChartEventParam {
        pub time: i64,
        pub logical: f64,
        pub point_x: f32,
        pub point_y: f32,
        pub price: f64,
    }

    pub type ChartEventCallback =
        extern "C" fn(param: *const ChartEventParam, user_data: *mut c_void);

    /// Opaque chart handle passed via C-ABI
    pub struct Chart {
        state: ChartState,
        scene: Scene,
        device: wgpu::Device,
        queue: wgpu::Queue,
        surface: wgpu::Surface<'static>,
        surface_config: wgpu::SurfaceConfiguration,
        vello_renderer: VelloRenderer,

        click_cb: Option<(ChartEventCallback, *mut c_void)>,
        crosshair_move_cb: Option<(ChartEventCallback, *mut c_void)>,
        dbl_click_cb: Option<(ChartEventCallback, *mut c_void)>,
    }

    // ----- Lifecycle -----

    #[no_mangle]
    pub extern "C" fn chart_create(
        width: u32,
        height: u32,
        scale_factor: f64,
        metal_layer: *mut c_void,
    ) -> *mut Chart {
        env_logger::try_init().ok();

        // Start with empty data — host should call chart_set_data() to provide bars
        let data = ChartData { bars: Vec::new() };
        let state = ChartState::new(data, width as f32, height as f32, scale_factor);

        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
            backends: wgpu::Backends::METAL,
            ..Default::default()
        });

        let surface = unsafe {
            instance
                .create_surface_unsafe(wgpu::SurfaceTargetUnsafe::CoreAnimationLayer(metal_layer))
                .expect("Failed to create wgpu surface from CAMetalLayer")
        };

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
            state,
            scene: Scene::new(),
            device,
            queue,
            surface,
            surface_config,
            vello_renderer,
            click_cb: None,
            crosshair_move_cb: None,
            dbl_click_cb: None,
        };

        Box::into_raw(Box::new(chart))
    }

    #[no_mangle]
    pub extern "C" fn chart_render(chart: *mut Chart) {
        let chart = unsafe {
            assert!(!chart.is_null());
            &mut *chart
        };

        chart.scene.reset();
        render_chart(&mut chart.scene, &chart.state);

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

    #[no_mangle]
    pub extern "C" fn chart_resize(chart: *mut Chart, width: u32, height: u32, scale_factor: f64) {
        let chart = unsafe {
            assert!(!chart.is_null());
            &mut *chart
        };

        chart
            .state
            .resize(width as f32, height as f32, scale_factor);

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

    #[no_mangle]
    pub extern "C" fn chart_destroy(chart: *mut Chart) {
        if !chart.is_null() {
            unsafe {
                drop(Box::from_raw(chart));
            }
        }
    }

    // ----- Interaction C-ABI (all return bool: needs_redraw) -----

    #[no_mangle]
    pub extern "C" fn chart_pointer_move(chart: *mut Chart, x: f32, y: f32) -> bool {
        let chart = unsafe {
            assert!(!chart.is_null());
            &mut *chart
        };
        let redraw = chart.state.pointer_move(x, y);

        if let Some((cb, user_data)) = chart.crosshair_move_cb {
            let logical = chart
                .state
                .time_scale
                .x_to_index(x, &chart.state.layout.plot_area);
            let nearest_idx = chart
                .state
                .time_scale
                .x_to_nearest_index(x, &chart.state.layout.plot_area);
            let time = nearest_idx
                .and_then(|i| chart.state.data.bars.get(i))
                .map(|b| b.time)
                .unwrap_or(0);
            let price = chart.state.panes[0]
                .price_scale
                .y_to_price(y, &chart.state.panes[0].layout_rect);

            let param = ChartEventParam {
                time,
                logical: logical as f64,
                point_x: x,
                point_y: y,
                price,
            };
            cb(&param, user_data);
        }

        redraw
    }

    #[no_mangle]
    pub extern "C" fn chart_pointer_down(chart: *mut Chart, x: f32, y: f32, button: u8) -> bool {
        let chart = unsafe {
            assert!(!chart.is_null());
            &mut *chart
        };
        chart.state.pointer_down(x, y, button)
    }

    #[no_mangle]
    pub extern "C" fn chart_pointer_up(chart: *mut Chart, x: f32, y: f32, button: u8) -> bool {
        let chart = unsafe {
            assert!(!chart.is_null());
            &mut *chart
        };
        let redraw = chart.state.pointer_up(x, y, button);

        if chart.state.pending_events.click.is_some() {
            if let Some((cb, user_data)) = chart.click_cb {
                let logical = chart
                    .state
                    .time_scale
                    .x_to_index(x, &chart.state.layout.plot_area);
                let nearest_idx = chart
                    .state
                    .time_scale
                    .x_to_nearest_index(x, &chart.state.layout.plot_area);
                let time = nearest_idx
                    .and_then(|i| chart.state.data.bars.get(i))
                    .map(|b| b.time)
                    .unwrap_or(0);
                let price = chart.state.panes[0]
                    .price_scale
                    .y_to_price(y, &chart.state.panes[0].layout_rect);

                let param = ChartEventParam {
                    time,
                    logical: logical as f64,
                    point_x: x,
                    point_y: y,
                    price,
                };
                cb(&param, user_data);
            }
            chart.state.pending_events.click = None;
        }

        // Dispatch dbl_click callback
        if chart.state.pending_events.dbl_click.is_some() {
            if let Some((cb, user_data)) = chart.dbl_click_cb {
                let logical = chart
                    .state
                    .time_scale
                    .x_to_index(x, &chart.state.layout.plot_area);
                let nearest_idx = chart
                    .state
                    .time_scale
                    .x_to_nearest_index(x, &chart.state.layout.plot_area);
                let time = nearest_idx
                    .and_then(|i| chart.state.data.bars.get(i))
                    .map(|b| b.time)
                    .unwrap_or(0);
                let price = chart.state.panes[0]
                    .price_scale
                    .y_to_price(y, &chart.state.panes[0].layout_rect);

                let param = ChartEventParam {
                    time,
                    logical: logical as f64,
                    point_x: x,
                    point_y: y,
                    price,
                };
                cb(&param, user_data);
            }
            chart.state.pending_events.dbl_click = None;
        }

        redraw
    }

    #[no_mangle]
    pub extern "C" fn chart_subscribe_click(
        chart: *mut Chart,
        callback: ChartEventCallback,
        user_data: *mut std::ffi::c_void,
    ) {
        let chart = unsafe {
            assert!(!chart.is_null());
            &mut *chart
        };
        chart.click_cb = Some((callback, user_data));
    }

    #[no_mangle]
    pub extern "C" fn chart_unsubscribe_click(chart: *mut Chart) {
        let chart = unsafe {
            assert!(!chart.is_null());
            &mut *chart
        };
        chart.click_cb = None;
    }

    #[no_mangle]
    pub extern "C" fn chart_subscribe_dbl_click(
        chart: *mut Chart,
        callback: ChartEventCallback,
        user_data: *mut std::ffi::c_void,
    ) {
        let chart = unsafe {
            assert!(!chart.is_null());
            &mut *chart
        };
        chart.dbl_click_cb = Some((callback, user_data));
    }

    #[no_mangle]
    pub extern "C" fn chart_unsubscribe_dbl_click(chart: *mut Chart) {
        let chart = unsafe {
            assert!(!chart.is_null());
            &mut *chart
        };
        chart.dbl_click_cb = None;
    }

    #[no_mangle]
    pub extern "C" fn chart_subscribe_crosshair_move(
        chart: *mut Chart,
        callback: ChartEventCallback,
        user_data: *mut std::ffi::c_void,
    ) {
        let chart = unsafe {
            assert!(!chart.is_null());
            &mut *chart
        };
        chart.crosshair_move_cb = Some((callback, user_data));
    }

    #[no_mangle]
    pub extern "C" fn chart_unsubscribe_crosshair_move(chart: *mut Chart) {
        let chart = unsafe {
            assert!(!chart.is_null());
            &mut *chart
        };
        chart.crosshair_move_cb = None;
    }

    #[no_mangle]
    pub extern "C" fn chart_pointer_leave(chart: *mut Chart) -> bool {
        let chart = unsafe {
            assert!(!chart.is_null());
            &mut *chart
        };
        chart.state.pointer_leave()
    }

    #[no_mangle]
    pub extern "C" fn chart_set_crosshair_position(
        chart: *mut Chart,
        price: f64,
        time: i64,
        series_id: u32,
    ) -> bool {
        let chart = unsafe {
            assert!(!chart.is_null());
            &mut *chart
        };
        chart.state.set_crosshair_position(price, time, series_id)
    }

    #[no_mangle]
    pub extern "C" fn chart_clear_crosshair_position(chart: *mut Chart) -> bool {
        let chart = unsafe {
            assert!(!chart.is_null());
            &mut *chart
        };
        chart.state.clear_crosshair_position()
    }

    #[no_mangle]
    pub extern "C" fn chart_scroll(chart: *mut Chart, delta_x: f32, delta_y: f32) -> bool {
        let chart = unsafe {
            assert!(!chart.is_null());
            &mut *chart
        };
        chart.state.scroll(delta_x, delta_y)
    }

    #[no_mangle]
    pub extern "C" fn chart_zoom(chart: *mut Chart, factor: f32, center_x: f32) -> bool {
        let chart = unsafe {
            assert!(!chart.is_null());
            &mut *chart
        };
        chart.state.zoom(factor, center_x)
    }

    #[no_mangle]
    pub extern "C" fn chart_pinch(
        chart: *mut Chart,
        scale: f32,
        center_x: f32,
        center_y: f32,
    ) -> bool {
        let chart = unsafe {
            assert!(!chart.is_null());
            &mut *chart
        };
        chart.state.pinch(scale, center_x, center_y)
    }

    #[no_mangle]
    pub extern "C" fn chart_fit_content(chart: *mut Chart) -> bool {
        let chart = unsafe {
            assert!(!chart.is_null());
            &mut *chart
        };
        chart.state.fit_content()
    }

    #[no_mangle]
    pub extern "C" fn chart_key_down(chart: *mut Chart, key_code: u32) -> bool {
        let chart = unsafe {
            assert!(!chart.is_null());
            &mut *chart
        };
        let key = crate::chart_state::ChartKey::from_code(key_code);
        chart.state.key_down(key)
    }

    #[no_mangle]
    pub extern "C" fn chart_tick(chart: *mut Chart) {
        let chart = unsafe {
            assert!(!chart.is_null());
            &mut *chart
        };
        chart.state.tick();
    }

    // ----- Data management C-ABI -----

    /// Set all bar data from a flat array of (time, open, high, low, close).
    /// `count` is the number of bars. `data` points to `count * 5` doubles.
    #[no_mangle]
    pub extern "C" fn chart_set_data(chart: *mut Chart, data: *const f64, count: u32) {
        let chart = unsafe {
            assert!(!chart.is_null());
            &mut *chart
        };
        let count = count as usize;
        let slice = unsafe { std::slice::from_raw_parts(data, count * 5) };
        let bars: Vec<crate::chart_model::OhlcBar> = (0..count)
            .map(|i| {
                let base = i * 5;
                crate::chart_model::OhlcBar {
                    time: slice[base] as i64,
                    open: slice[base + 1],
                    high: slice[base + 2],
                    low: slice[base + 3],
                    close: slice[base + 4],
                }
            })
            .collect();
        chart.state.set_data(bars);
    }

    /// Update or insert a single bar.
    #[no_mangle]
    pub extern "C" fn chart_update_bar(
        chart: *mut Chart,
        time: i64,
        open: f64,
        high: f64,
        low: f64,
        close: f64,
    ) -> bool {
        let chart = unsafe {
            assert!(!chart.is_null());
            &mut *chart
        };
        chart.state.update_bar(crate::chart_model::OhlcBar {
            time,
            open,
            high,
            low,
            close,
        });
        true
    }

    /// Update or insert a single OHLC bar for a specific series.
    #[no_mangle]
    pub extern "C" fn chart_series_update_ohlc_bar(
        chart: *mut Chart,
        series_id: u32,
        time: i64,
        open: f64,
        high: f64,
        low: f64,
        close: f64,
    ) -> bool {
        let chart = unsafe {
            assert!(!chart.is_null());
            &mut *chart
        };
        if let Some(series) = chart.state.series.get_mut(series_id) {
            series.data.update_ohlc(crate::chart_model::OhlcBar {
                time,
                open,
                high,
                low,
                close,
            });
            return true;
        }
        false
    }

    /// Update or insert a single line/area/baseline point for a specific series.
    #[no_mangle]
    pub extern "C" fn chart_series_update_line_bar(
        chart: *mut Chart,
        series_id: u32,
        time: i64,
        value: f64,
    ) -> bool {
        let chart = unsafe {
            assert!(!chart.is_null());
            &mut *chart
        };
        if let Some(series) = chart.state.series.get_mut(series_id) {
            series
                .data
                .update_line(crate::series::LineDataPoint { time, value });
            return true;
        }
        false
    }

    /// Update or insert a single histogram bar for a specific series.
    #[no_mangle]
    pub extern "C" fn chart_series_update_histogram_bar(
        chart: *mut Chart,
        series_id: u32,
        time: i64,
        value: f64,
        color_rgba: u32,
        has_color: bool,
    ) -> bool {
        let chart = unsafe {
            assert!(!chart.is_null());
            &mut *chart
        };
        if let Some(series) = chart.state.series.get_mut(series_id) {
            let color = if has_color {
                let bytes = color_rgba.to_be_bytes();
                Some([
                    bytes[0] as f32 / 255.0,
                    bytes[1] as f32 / 255.0,
                    bytes[2] as f32 / 255.0,
                    bytes[3] as f32 / 255.0,
                ])
            } else {
                None
            };
            series
                .data
                .update_histogram(crate::series::HistogramDataPoint { time, value, color });
            return true;
        }
        false
    }

    /// Remove `count` data items from the end of a series.
    #[no_mangle]
    pub extern "C" fn chart_series_pop(chart: *mut Chart, series_id: u32, count: u32) -> bool {
        let chart = unsafe {
            assert!(!chart.is_null());
            &mut *chart
        };
        if let Some(series) = chart.state.series.get_mut(series_id) {
            series.data.pop(count as usize);
            return true;
        }
        false
    }

    /// Get the number of bars.
    #[no_mangle]
    pub extern "C" fn chart_bar_count(chart: *mut Chart) -> u32 {
        let chart = unsafe {
            assert!(!chart.is_null());
            &*chart
        };
        chart.state.bar_count() as u32
    }

    /// Set the active series type. 0=OHLC, 1=Candlestick, 2=Line.
    #[no_mangle]
    pub extern "C" fn chart_set_series_type(chart: *mut Chart, series_type: u32) -> bool {
        let chart = unsafe {
            assert!(!chart.is_null());
            &mut *chart
        };
        chart.state.active_series_type = match series_type {
            0 => crate::series::SeriesType::Ohlc,
            1 => crate::series::SeriesType::Candlestick,
            2 => crate::series::SeriesType::Line,
            3 => crate::series::SeriesType::Area,
            4 => crate::series::SeriesType::Histogram,
            5 => crate::series::SeriesType::Baseline,
            _ => return false,
        };
        true
    }

    /// Apply options to the chart globally from a JSON string.
    #[unsafe(no_mangle)]
    pub extern "C" fn chart_apply_options(
        chart: *mut Chart,
        json_cstr: *const std::os::raw::c_char,
    ) -> bool {
        let chart = unsafe {
            assert!(!chart.is_null());
            &mut *chart
        };
        if json_cstr.is_null() {
            return false;
        }
        let json_str = unsafe { std::ffi::CStr::from_ptr(json_cstr) }.to_string_lossy();

        if chart.state.options.apply_json(&json_str) {
            chart.state.update_price_scale();
            return true;
        }
        false
    }

    /// Apply options to a specific series from a JSON string.
    #[unsafe(no_mangle)]
    pub extern "C" fn chart_series_apply_options(
        chart: *mut Chart,
        series_id: u32,
        json_cstr: *const std::os::raw::c_char,
    ) -> bool {
        let chart = unsafe {
            assert!(!chart.is_null());
            &mut *chart
        };
        if json_cstr.is_null() {
            return false;
        }
        let json_str = unsafe { std::ffi::CStr::from_ptr(json_cstr) }.to_string_lossy();

        if let Some(series) = chart.state.series.get_mut(series_id) {
            return series.apply_options_json(&json_str);
        }
        false
    }

    #[unsafe(no_mangle)]
    pub extern "C" fn chart_series_create_price_line(
        chart: *mut Chart,
        series_id: u32,
        options_json_cstr: *const std::os::raw::c_char,
    ) -> u32 {
        let chart = unsafe {
            assert!(!chart.is_null());
            &mut *chart
        };
        if let Some(series) = chart.state.series.get_mut(series_id) {
            let mut opts = crate::series::PriceLineOptions::default();
            if !options_json_cstr.is_null() {
                let json_str =
                    unsafe { std::ffi::CStr::from_ptr(options_json_cstr) }.to_string_lossy();
                if let Ok(partial) = serde_json::from_str::<serde_json::Value>(&json_str) {
                    if let Ok(mut full) = serde_json::to_value(&opts) {
                        crate::chart_options::merge_json(&mut full, partial);
                        if let Ok(new_opts) = serde_json::from_value(full) {
                            opts = new_opts;
                        }
                    }
                }
            }
            return series.add_price_line(opts);
        }
        u32::MAX
    }

    #[no_mangle]
    pub extern "C" fn chart_series_remove_price_line(
        chart: *mut Chart,
        series_id: u32,
        line_id: u32,
    ) -> bool {
        let chart = unsafe {
            assert!(!chart.is_null());
            &mut *chart
        };
        if let Some(series) = chart.state.series.get_mut(series_id) {
            return series.remove_price_line(line_id);
        }
        false
    }

    // ----- Read & Coordinate Translation APIs -----

    #[no_mangle]
    pub extern "C" fn chart_price_to_coordinate(chart: *mut Chart, price: f64) -> f32 {
        let chart = unsafe {
            assert!(!chart.is_null());
            &mut *chart
        };
        chart.state.panes[0]
            .price_scale
            .price_to_y(price, &chart.state.panes[0].layout_rect)
    }

    #[no_mangle]
    pub extern "C" fn chart_coordinate_to_price(chart: *mut Chart, y: f32) -> f64 {
        let chart = unsafe {
            assert!(!chart.is_null());
            &mut *chart
        };
        chart.state.panes[0]
            .price_scale
            .y_to_price(y, &chart.state.panes[0].layout_rect)
    }

    #[no_mangle]
    pub extern "C" fn chart_logical_to_coordinate(chart: *mut Chart, logical: f64) -> f32 {
        let chart = unsafe {
            assert!(!chart.is_null());
            &mut *chart
        };
        chart
            .state
            .time_scale
            .logical_to_x(logical as f32, &chart.state.layout.plot_area)
    }

    #[no_mangle]
    pub extern "C" fn chart_coordinate_to_logical(chart: *mut Chart, x: f32) -> f64 {
        let chart = unsafe {
            assert!(!chart.is_null());
            &mut *chart
        };
        chart
            .state
            .time_scale
            .x_to_index(x, &chart.state.layout.plot_area) as f64
    }

    #[no_mangle]
    pub extern "C" fn chart_time_to_coordinate(chart: *mut Chart, time: i64) -> f32 {
        let chart = unsafe {
            assert!(!chart.is_null());
            &mut *chart
        };
        let idx = match chart
            .state
            .data
            .bars
            .binary_search_by_key(&time, |b| b.time)
        {
            Ok(i) => i as f32,
            Err(i) => {
                if i < chart.state.data.bars.len() {
                    i as f32
                } else {
                    chart.state.data.bars.len().saturating_sub(1) as f32
                }
            }
        };
        chart
            .state
            .time_scale
            .logical_to_x(idx, &chart.state.layout.plot_area)
    }

    #[no_mangle]
    pub extern "C" fn chart_coordinate_to_time(chart: *mut Chart, x: f32) -> i64 {
        let chart = unsafe {
            assert!(!chart.is_null());
            &mut *chart
        };
        if let Some(idx) = chart
            .state
            .time_scale
            .x_to_nearest_index(x, &chart.state.layout.plot_area)
        {
            if let Some(b) = chart.state.data.bars.get(idx) {
                return b.time;
            }
        }
        0
    }

    // ----- Data Retrieval APIs -----

    #[no_mangle]
    pub extern "C" fn chart_series_data_length(chart: *mut Chart, series_id: u32) -> u32 {
        let chart = unsafe {
            assert!(!chart.is_null());
            &mut *chart
        };
        if let Some(series) = chart.state.series.get(series_id) {
            match &series.data {
                crate::series::SeriesData::Ohlc(bars) => bars.len() as u32,
                crate::series::SeriesData::Line(pts) => pts.len() as u32,
                crate::series::SeriesData::Histogram(pts) => pts.len() as u32,
            }
        } else {
            0
        }
    }

    #[no_mangle]
    pub extern "C" fn chart_series_get_ohlc_data(
        chart: *mut Chart,
        series_id: u32,
        times: *mut i64,
        opens: *mut f64,
        highs: *mut f64,
        lows: *mut f64,
        closes: *mut f64,
        max_count: u32,
    ) -> u32 {
        let chart = unsafe {
            assert!(!chart.is_null());
            &mut *chart
        };
        if let Some(series) = chart.state.series.get(series_id) {
            if let crate::series::SeriesData::Ohlc(bars) = &series.data {
                let count = (max_count as usize).min(bars.len());
                unsafe {
                    let mut times = times;
                    let mut opens = opens;
                    let mut highs = highs;
                    let mut lows = lows;
                    let mut closes = closes;
                    for i in 0..count {
                        if !times.is_null() {
                            *times = bars[i].time;
                            times = times.add(1);
                        }
                        if !opens.is_null() {
                            *opens = bars[i].open;
                            opens = opens.add(1);
                        }
                        if !highs.is_null() {
                            *highs = bars[i].high;
                            highs = highs.add(1);
                        }
                        if !lows.is_null() {
                            *lows = bars[i].low;
                            lows = lows.add(1);
                        }
                        if !closes.is_null() {
                            *closes = bars[i].close;
                            closes = closes.add(1);
                        }
                    }
                }
                return count as u32;
            }
        }
        0
    }

    #[no_mangle]
    pub extern "C" fn chart_series_get_line_data(
        chart: *mut Chart,
        series_id: u32,
        times: *mut i64,
        values: *mut f64,
        max_count: u32,
    ) -> u32 {
        let chart = unsafe {
            assert!(!chart.is_null());
            &mut *chart
        };
        if let Some(series) = chart.state.series.get(series_id) {
            if let crate::series::SeriesData::Line(pts) = &series.data {
                let count = (max_count as usize).min(pts.len());
                unsafe {
                    let mut times = times;
                    let mut values = values;
                    for i in 0..count {
                        if !times.is_null() {
                            *times = pts[i].time;
                            times = times.add(1);
                        }
                        if !values.is_null() {
                            *values = pts[i].value;
                            values = values.add(1);
                        }
                    }
                }
                return count as u32;
            }
        }
        0
    }

    #[no_mangle]
    pub extern "C" fn chart_series_get_histogram_data(
        chart: *mut Chart,
        series_id: u32,
        times: *mut i64,
        values: *mut f64,
        max_count: u32,
    ) -> u32 {
        let chart = unsafe {
            assert!(!chart.is_null());
            &mut *chart
        };
        if let Some(series) = chart.state.series.get(series_id) {
            if let crate::series::SeriesData::Histogram(pts) = &series.data {
                let count = (max_count as usize).min(pts.len());
                unsafe {
                    let mut times = times;
                    let mut values = values;
                    for i in 0..count {
                        if !times.is_null() {
                            *times = pts[i].time;
                            times = times.add(1);
                        }
                        if !values.is_null() {
                            *values = pts[i].value;
                            values = values.add(1);
                        }
                    }
                }
                return count as u32;
            }
        }
        0
    }

    #[no_mangle]
    pub extern "C" fn chart_series_get_last_value_data(
        chart: *mut Chart,
        series_id: u32,
        out_time: *mut i64,
        out_value: *mut f64,
    ) -> bool {
        let chart = unsafe {
            assert!(!chart.is_null());
            &mut *chart
        };
        if let Some(series) = chart.state.series.get(series_id) {
            match &series.data {
                crate::series::SeriesData::Ohlc(bars) => {
                    if let Some(b) = bars.last() {
                        unsafe {
                            if !out_time.is_null() {
                                *out_time = b.time;
                            }
                            if !out_value.is_null() {
                                *out_value = b.close;
                            }
                        }
                        return true;
                    }
                }
                crate::series::SeriesData::Line(pts) => {
                    if let Some(p) = pts.last() {
                        unsafe {
                            if !out_time.is_null() {
                                *out_time = p.time;
                            }
                            if !out_value.is_null() {
                                *out_value = p.value;
                            }
                        }
                        return true;
                    }
                }
                crate::series::SeriesData::Histogram(pts) => {
                    if let Some(p) = pts.last() {
                        unsafe {
                            if !out_time.is_null() {
                                *out_time = p.time;
                            }
                            if !out_value.is_null() {
                                *out_value = p.value;
                            }
                        }
                        return true;
                    }
                }
            }
        }
        false
    }

    /// Set OHLC data for a specific series.
    #[unsafe(no_mangle)]
    pub extern "C" fn chart_series_set_ohlc_data(
        chart: *mut Chart,
        series_id: u32,
        times: *const i64,
        opens: *const f64,
        highs: *const f64,
        lows: *const f64,
        closes: *const f64,
        count: u32,
    ) -> bool {
        let chart = unsafe {
            assert!(!chart.is_null());
            &mut *chart
        };
        let count = count as usize;
        let times = unsafe { std::slice::from_raw_parts(times, count) };
        let opens = unsafe { std::slice::from_raw_parts(opens, count) };
        let highs = unsafe { std::slice::from_raw_parts(highs, count) };
        let lows = unsafe { std::slice::from_raw_parts(lows, count) };
        let closes = unsafe { std::slice::from_raw_parts(closes, count) };

        let mut bars = Vec::with_capacity(count);
        for i in 0..count {
            bars.push(crate::chart_model::OhlcBar {
                time: times[i],
                open: opens[i],
                high: highs[i],
                low: lows[i],
                close: closes[i],
            });
        }
        if let Some(series) = chart.state.series.get_mut(series_id) {
            series.data.set_ohlc(bars);
            return true;
        }
        false
    }

    /// Set Line/Area/Baseline data for a specific series.
    #[unsafe(no_mangle)]
    pub extern "C" fn chart_series_set_line_data(
        chart: *mut Chart,
        series_id: u32,
        times: *const i64,
        values: *const f64,
        count: u32,
    ) -> bool {
        let chart = unsafe {
            assert!(!chart.is_null());
            &mut *chart
        };
        let count = count as usize;
        let times = unsafe { std::slice::from_raw_parts(times, count) };
        let values = unsafe { std::slice::from_raw_parts(values, count) };

        let mut pts = Vec::with_capacity(count);
        for i in 0..count {
            pts.push(crate::series::LineDataPoint {
                time: times[i],
                value: values[i],
            });
        }
        if let Some(series) = chart.state.series.get_mut(series_id) {
            series.data.set_line(pts);
            return true;
        }
        false
    }

    /// Set Histogram data for a specific series.
    #[unsafe(no_mangle)]
    pub extern "C" fn chart_series_set_histogram_data(
        chart: *mut Chart,
        series_id: u32,
        times: *const i64,
        values: *const f64,
        colors: *const u32,
        count: u32,
    ) -> bool {
        let chart = unsafe {
            assert!(!chart.is_null());
            &mut *chart
        };
        let count = count as usize;
        let times = unsafe { std::slice::from_raw_parts(times, count) };
        let values = unsafe { std::slice::from_raw_parts(values, count) };
        let colors = if colors.is_null() {
            None
        } else {
            Some(unsafe { std::slice::from_raw_parts(colors, count) })
        };

        let mut pts = Vec::with_capacity(count);
        for i in 0..count {
            let color = if let Some(c) = colors {
                let bytes = c[i].to_be_bytes();
                Some([
                    bytes[0] as f32 / 255.0,
                    bytes[1] as f32 / 255.0,
                    bytes[2] as f32 / 255.0,
                    bytes[3] as f32 / 255.0,
                ])
            } else {
                None
            };
            pts.push(crate::series::HistogramDataPoint {
                time: times[i],
                value: values[i],
                color,
            });
        }
        if let Some(series) = chart.state.series.get_mut(series_id) {
            series.data.set_histogram(pts);
            return true;
        }
        false
    }

    /// Add a new line series to the chart from an array of (time, value) pairs.
    /// Returns the assigned series ID. Returns u32::MAX on error.
    #[unsafe(no_mangle)]
    pub extern "C" fn chart_add_line_series(
        chart: *mut Chart,
        times: *const i64,
        values: *const f64,
        count: u32,
    ) -> u32 {
        let chart = unsafe {
            assert!(!chart.is_null());
            &mut *chart
        };
        let times = unsafe { std::slice::from_raw_parts(times, count as usize) };
        let values = unsafe { std::slice::from_raw_parts(values, count as usize) };

        let points: Vec<crate::series::LineDataPoint> = times
            .iter()
            .zip(values.iter())
            .map(|(&t, &v)| crate::series::LineDataPoint { time: t, value: v })
            .collect();

        let series = crate::series::Series::line(0, points);
        chart.state.series.add(series)
    }

    /// Add a new OHLC series to the chart.
    #[unsafe(no_mangle)]
    pub extern "C" fn chart_add_ohlc_series(
        chart: *mut Chart,
        times: *const i64,
        opens: *const f64,
        highs: *const f64,
        lows: *const f64,
        closes: *const f64,
        count: u32,
    ) -> u32 {
        let chart = unsafe {
            assert!(!chart.is_null());
            &mut *chart
        };
        let times = unsafe { std::slice::from_raw_parts(times, count as usize) };
        let opens = unsafe { std::slice::from_raw_parts(opens, count as usize) };
        let highs = unsafe { std::slice::from_raw_parts(highs, count as usize) };
        let lows = unsafe { std::slice::from_raw_parts(lows, count as usize) };
        let closes = unsafe { std::slice::from_raw_parts(closes, count as usize) };

        let mut bars = Vec::with_capacity(count as usize);
        for i in 0..(count as usize) {
            bars.push(crate::chart_model::OhlcBar {
                time: times[i],
                open: opens[i],
                high: highs[i],
                low: lows[i],
                close: closes[i],
            });
        }

        let series = crate::series::Series::ohlc(0, bars);
        chart.state.series.add(series)
    }

    /// Add a new Candlestick series to the chart.
    #[unsafe(no_mangle)]
    pub extern "C" fn chart_add_candlestick_series(
        chart: *mut Chart,
        times: *const i64,
        opens: *const f64,
        highs: *const f64,
        lows: *const f64,
        closes: *const f64,
        count: u32,
    ) -> u32 {
        let chart = unsafe {
            assert!(!chart.is_null());
            &mut *chart
        };
        let times = unsafe { std::slice::from_raw_parts(times, count as usize) };
        let opens = unsafe { std::slice::from_raw_parts(opens, count as usize) };
        let highs = unsafe { std::slice::from_raw_parts(highs, count as usize) };
        let lows = unsafe { std::slice::from_raw_parts(lows, count as usize) };
        let closes = unsafe { std::slice::from_raw_parts(closes, count as usize) };

        let mut bars = Vec::with_capacity(count as usize);
        for i in 0..(count as usize) {
            bars.push(crate::chart_model::OhlcBar {
                time: times[i],
                open: opens[i],
                high: highs[i],
                low: lows[i],
                close: closes[i],
            });
        }

        let series = crate::series::Series::candlestick(0, bars);
        chart.state.series.add(series)
    }

    /// Add a new area series to the chart.
    #[unsafe(no_mangle)]
    pub extern "C" fn chart_add_area_series(
        chart: *mut Chart,
        times: *const i64,
        values: *const f64,
        count: u32,
    ) -> u32 {
        let chart = unsafe {
            assert!(!chart.is_null());
            &mut *chart
        };
        let times = unsafe { std::slice::from_raw_parts(times, count as usize) };
        let values = unsafe { std::slice::from_raw_parts(values, count as usize) };

        let points: Vec<crate::series::LineDataPoint> = times
            .iter()
            .zip(values.iter())
            .map(|(&t, &v)| crate::series::LineDataPoint { time: t, value: v })
            .collect();

        let series = crate::series::Series::area(0, points);
        chart.state.series.add(series)
    }

    /// Add a new baseline series to the chart.
    #[unsafe(no_mangle)]
    pub extern "C" fn chart_add_baseline_series(
        chart: *mut Chart,
        times: *const i64,
        values: *const f64,
        count: u32,
        base_value: f64,
    ) -> u32 {
        let chart = unsafe {
            assert!(!chart.is_null());
            &mut *chart
        };
        let times = unsafe { std::slice::from_raw_parts(times, count as usize) };
        let values = unsafe { std::slice::from_raw_parts(values, count as usize) };

        let points: Vec<crate::series::LineDataPoint> = times
            .iter()
            .zip(values.iter())
            .map(|(&t, &v)| crate::series::LineDataPoint { time: t, value: v })
            .collect();

        let series = crate::series::Series::baseline(0, points, base_value);
        chart.state.series.add(series)
    }

    /// Add a new histogram series to the chart.
    /// `colors` can be null. If provided, length must equal `count`. Format: 0xRRGGBBAA.
    #[unsafe(no_mangle)]
    pub extern "C" fn chart_add_histogram_series(
        chart: *mut Chart,
        times: *const i64,
        values: *const f64,
        colors: *const u32,
        count: u32,
    ) -> u32 {
        let chart = unsafe {
            assert!(!chart.is_null());
            &mut *chart
        };
        let times = unsafe { std::slice::from_raw_parts(times, count as usize) };
        let values = unsafe { std::slice::from_raw_parts(values, count as usize) };
        let colors_slice = if colors.is_null() || count == 0 {
            None
        } else {
            Some(unsafe { std::slice::from_raw_parts(colors, count as usize) })
        };

        let mut points = Vec::with_capacity(count as usize);
        for i in 0..(count as usize) {
            let color = colors_slice.map(|slice| {
                let c = slice[i];
                let r = ((c >> 24) & 0xFF) as f32 / 255.0;
                let g = ((c >> 16) & 0xFF) as f32 / 255.0;
                let b = ((c >> 8) & 0xFF) as f32 / 255.0;
                let a = (c & 0xFF) as f32 / 255.0;
                [r, g, b, a]
            });
            points.push(crate::series::HistogramDataPoint {
                time: times[i],
                value: values[i],
                color,
            });
        }

        let series = crate::series::Series::histogram(0, points);
        chart.state.series.add(series)
    }

    /// Remove a series by its ID. Returns true if the series was found and removed.
    #[unsafe(no_mangle)]
    pub extern "C" fn chart_remove_series(chart: *mut Chart, series_id: u32) -> bool {
        let chart = unsafe {
            assert!(!chart.is_null());
            &mut *chart
        };
        chart.state.series.remove(series_id)
    }

    /// Get the number of additional series on the chart.
    #[unsafe(no_mangle)]
    pub extern "C" fn chart_series_count(chart: *const Chart) -> u32 {
        let chart = unsafe {
            assert!(!chart.is_null());
            &*chart
        };
        chart.state.series.len() as u32
    }

    // ----- Pane Management C-ABIs -----

    /// Add a new pane to the chart. Returns the pane ID.
    /// `height_stretch` controls relative height (1.0 = equal to other panes).
    #[unsafe(no_mangle)]
    pub extern "C" fn chart_add_pane(chart: *mut Chart, height_stretch: f32) -> u32 {
        let chart = unsafe {
            assert!(!chart.is_null());
            &mut *chart
        };
        chart.state.add_pane(height_stretch)
    }

    /// Remove a pane by ID. Returns true if removed.
    /// Pane 0 (main) cannot be removed. Orphaned series move to pane 0.
    #[unsafe(no_mangle)]
    pub extern "C" fn chart_remove_pane(chart: *mut Chart, pane_id: u32) -> bool {
        let chart = unsafe {
            assert!(!chart.is_null());
            &mut *chart
        };
        chart.state.remove_pane(pane_id)
    }

    /// Move a series to a specific pane (by pane ID).
    /// Returns true if both the series and pane were found.
    #[unsafe(no_mangle)]
    pub extern "C" fn chart_series_move_to_pane(
        chart: *mut Chart,
        series_id: u32,
        pane_id: u32,
    ) -> bool {
        let chart = unsafe {
            assert!(!chart.is_null());
            &mut *chart
        };
        chart.state.move_series_to_pane(series_id, pane_id)
    }

    /// Get the number of panes.
    #[unsafe(no_mangle)]
    pub extern "C" fn chart_pane_count(chart: *const Chart) -> u32 {
        let chart = unsafe {
            assert!(!chart.is_null());
            &*chart
        };
        chart.state.panes.len() as u32
    }

    /// Swap two panes by their IDs.
    #[unsafe(no_mangle)]
    pub extern "C" fn chart_swap_panes(chart: *mut Chart, pane_id_a: u32, pane_id_b: u32) -> bool {
        let chart = unsafe {
            assert!(!chart.is_null());
            &mut *chart
        };
        chart.state.swap_panes(pane_id_a, pane_id_b)
    }

    /// Get the layout rect of a pane by ID.
    /// Returns true if pane exists, writing x/y/width/height to the out pointers.
    #[unsafe(no_mangle)]
    pub extern "C" fn chart_pane_size(
        chart: *const Chart,
        pane_id: u32,
        out_x: *mut f32,
        out_y: *mut f32,
        out_width: *mut f32,
        out_height: *mut f32,
    ) -> bool {
        let chart = unsafe {
            assert!(!chart.is_null());
            &*chart
        };
        if let Some((x, y, w, h)) = chart.state.pane_size(pane_id) {
            unsafe {
                if !out_x.is_null() {
                    *out_x = x;
                }
                if !out_y.is_null() {
                    *out_y = y;
                }
                if !out_width.is_null() {
                    *out_width = w;
                }
                if !out_height.is_null() {
                    *out_height = h;
                }
            }
            true
        } else {
            false
        }
    }
}
