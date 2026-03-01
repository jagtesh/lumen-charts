pub mod chart_model;
pub mod chart_options;
pub mod chart_renderer;
pub mod chart_state;
pub mod data_layer;
pub mod formatters;
pub mod invalidation;
pub mod overlays;
pub mod price_scale;
pub mod sample_data;
pub mod scale;
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
    use crate::chart_renderer::{render_bottom_scene, render_crosshair_scene};
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

    /// Chart handle containing GPU context and chart state.
    /// Fields are public to allow platform-specific initialization (e.g., WASM canvas).
    pub struct Chart {
        pub state: ChartState,
        pub scene: Scene,
        pub device: wgpu::Device,
        pub queue: wgpu::Queue,
        pub surface: wgpu::Surface<'static>,
        pub surface_config: wgpu::SurfaceConfiguration,
        pub vello_renderer: VelloRenderer,

        /// Cached bottom scene (background + grid + series + axes).
        /// Reused when only the crosshair changes.
        pub cached_bottom_scene: Option<Scene>,

        pub click_cb: Option<(ChartEventCallback, *mut c_void)>,
        pub crosshair_move_cb: Option<(ChartEventCallback, *mut c_void)>,
        pub dbl_click_cb: Option<(ChartEventCallback, *mut c_void)>,
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
            cached_bottom_scene: None,
            click_cb: None,
            crosshair_move_cb: None,
            dbl_click_cb: None,
        };

        Box::into_raw(Box::new(chart))
    }

    /// Internal render implementation shared by both explicit and conditional paths.
    fn render_internal(chart: &mut Chart, level: crate::invalidation::InvalidationLevel) {
        chart.scene.reset();

        if level.needs_bottom_scene() {
            // Light or Full — rebuild the bottom scene
            let mut bottom = Scene::new();
            render_bottom_scene(&mut bottom, &chart.state);
            chart.cached_bottom_scene = Some(bottom.clone());
            chart.scene.append(&bottom, None);
            chart.state.bottom_render_count += 1;
        } else if let Some(ref cached) = chart.cached_bottom_scene {
            // Cursor only — reuse cached bottom scene
            chart.scene.append(cached, None);
        } else {
            // No cache yet — must do full render
            let mut bottom = Scene::new();
            render_bottom_scene(&mut bottom, &chart.state);
            chart.cached_bottom_scene = Some(bottom.clone());
            chart.scene.append(&bottom, None);
            chart.state.bottom_render_count += 1;
        }

        // Always render crosshair on top
        render_crosshair_scene(&mut chart.scene, &chart.state);
        chart.state.crosshair_render_count += 1;

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

    /// Render the chart unconditionally. Call this after explicit state mutations
    /// to ensure the display is updated immediately.
    #[no_mangle]
    pub extern "C" fn chart_render(chart: *mut Chart) {
        let chart = unsafe {
            assert!(!chart.is_null());
            &mut *chart
        };

        // Consume any pending mask, but always render regardless
        let mask = chart.state.consume_mask();
        let level = mask.global_level();
        // Use Full if nothing was pending — ensures a complete render
        let effective_level = if level == crate::invalidation::InvalidationLevel::None {
            crate::invalidation::InvalidationLevel::Full
        } else {
            level
        };

        render_internal(chart, effective_level);
    }

    /// Render the chart only if the invalidation mask indicates a redraw is needed.
    /// Use this in event loops / display links to avoid unnecessary GPU work.
    #[no_mangle]
    pub extern "C" fn chart_render_if_needed(chart: *mut Chart) -> bool {
        let chart = unsafe {
            assert!(!chart.is_null());
            &mut *chart
        };

        let mask = chart.state.consume_mask();
        if !mask.needs_redraw() {
            chart.state.skipped_render_count += 1;
            return false;
        }

        render_internal(chart, mask.global_level());
        true
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

    // ----- Touch events -----

    #[no_mangle]
    pub extern "C" fn chart_touch_start(chart: *mut Chart, id: u32, x: f32, y: f32) -> bool {
        let chart = unsafe {
            assert!(!chart.is_null());
            &mut *chart
        };
        chart
            .state
            .touch_start(crate::chart_state::TouchPoint { id, x, y })
    }

    #[no_mangle]
    pub extern "C" fn chart_touch_move(chart: *mut Chart, id: u32, x: f32, y: f32) -> bool {
        let chart = unsafe {
            assert!(!chart.is_null());
            &mut *chart
        };
        chart
            .state
            .touch_move(crate::chart_state::TouchPoint { id, x, y })
    }

    /// Returns the recognized gesture as a u8:
    /// 0 = None, 1 = Pan, 2 = Pinch, 3 = Tap, 4 = LongPress
    #[no_mangle]
    pub extern "C" fn chart_touch_end(chart: *mut Chart, id: u32) -> u8 {
        let chart = unsafe {
            assert!(!chart.is_null());
            &mut *chart
        };
        let gesture = chart.state.touch_end(id);
        match gesture {
            crate::chart_state::TouchGesture::None => 0,
            crate::chart_state::TouchGesture::Pan => 1,
            crate::chart_state::TouchGesture::Pinch => 2,
            crate::chart_state::TouchGesture::Tap => 3,
            crate::chart_state::TouchGesture::LongPress => 4,
        }
    }

    /// Advance touch timers (call once per frame for long-press detection)
    #[no_mangle]
    pub extern "C" fn chart_touch_tick(chart: *mut Chart) {
        let chart = unsafe {
            assert!(!chart.is_null());
            &mut *chart
        };
        chart.state.touch_tick();
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
            chart.state.series_data_changed();
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
            chart.state.series_data_changed();
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
            chart.state.series_data_changed();
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
            chart.state.series_data_changed();
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
        chart
            .state
            .pending_mask
            .set_global(crate::invalidation::InvalidationLevel::Full);
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
            chart
                .state
                .pending_mask
                .set_global(crate::invalidation::InvalidationLevel::Full);
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
            let result = series.apply_options_json(&json_str);
            if result {
                chart
                    .state
                    .pending_mask
                    .set_global(crate::invalidation::InvalidationLevel::Full);
            }
            return result;
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
            let id = series.add_price_line(opts);
            chart
                .state
                .pending_mask
                .set_global(crate::invalidation::InvalidationLevel::Light);
            return id;
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
            let removed = series.remove_price_line(line_id);
            if removed {
                chart
                    .state
                    .pending_mask
                    .set_global(crate::invalidation::InvalidationLevel::Light);
            }
            return removed;
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
        chart.state.add_series(series)
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
        chart.state.add_series(series)
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
        chart.state.add_series(series)
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
        chart.state.add_series(series)
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
        chart.state.add_series(series)
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
        chart.state.add_series(series)
    }

    /// Remove a series by its ID. Returns true if the series was found and removed.
    #[unsafe(no_mangle)]
    pub extern "C" fn chart_remove_series(chart: *mut Chart, series_id: u32) -> bool {
        let chart = unsafe {
            assert!(!chart.is_null());
            &mut *chart
        };
        chart.state.remove_series(series_id)
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

    // ===================================================================
    // IChartApi — options getter
    // ===================================================================

    /// Get current chart options as JSON string.
    /// Caller must free the returned string with chart_free_string.
    #[unsafe(no_mangle)]
    pub extern "C" fn chart_get_options(chart: *const Chart) -> *mut std::os::raw::c_char {
        let chart = unsafe {
            assert!(!chart.is_null());
            &*chart
        };
        let json = serde_json::to_string(&chart.state.options).unwrap_or_default();
        std::ffi::CString::new(json).unwrap_or_default().into_raw()
    }

    /// Free a string returned by chart_get_options.
    #[unsafe(no_mangle)]
    pub extern "C" fn chart_free_string(s: *mut std::os::raw::c_char) {
        if !s.is_null() {
            unsafe {
                let _ = std::ffi::CString::from_raw(s);
            }
        }
    }

    // ===================================================================
    // ITimeScaleApi
    // ===================================================================

    /// Scroll to a specific bar position (fractional index from the right).
    /// position > 0 = empty space at right, < 0 = scrolled into history.
    #[unsafe(no_mangle)]
    pub extern "C" fn chart_time_scale_scroll_to_position(
        chart: *mut Chart,
        position: f32,
    ) -> bool {
        let chart = unsafe {
            assert!(!chart.is_null());
            &mut *chart
        };
        chart.state.time_scale.scroll_offset = position;
        chart.state.time_scale.clamp_scroll();
        chart
            .state
            .pending_mask
            .set_global(crate::invalidation::InvalidationLevel::Light);
        true
    }

    /// Scroll so the last bar is visible (right edge).
    #[unsafe(no_mangle)]
    pub extern "C" fn chart_time_scale_scroll_to_real_time(chart: *mut Chart) -> bool {
        let chart = unsafe {
            assert!(!chart.is_null());
            &mut *chart
        };
        chart.state.time_scale.scroll_offset = 0.0;
        chart
            .state
            .pending_mask
            .set_global(crate::invalidation::InvalidationLevel::Light);
        true
    }

    /// Get the visible time range (start_time, end_time) as unix timestamps.
    /// Returns true if data exists, writing to out pointers.
    #[unsafe(no_mangle)]
    pub extern "C" fn chart_time_scale_get_visible_range(
        chart: *const Chart,
        out_start: *mut i64,
        out_end: *mut i64,
    ) -> bool {
        let chart = unsafe {
            assert!(!chart.is_null());
            &*chart
        };
        if chart.state.data.bars.is_empty() {
            return false;
        }
        let (first, last) = chart
            .state
            .time_scale
            .visible_range(chart.state.layout.plot_area.width);
        let start_time = chart
            .state
            .data
            .bars
            .get(first)
            .map(|b| b.time)
            .unwrap_or(0);
        let end_time = chart
            .state
            .data
            .bars
            .get(last.saturating_sub(1))
            .map(|b| b.time)
            .unwrap_or(0);
        unsafe {
            if !out_start.is_null() {
                *out_start = start_time;
            }
            if !out_end.is_null() {
                *out_end = end_time;
            }
        }
        true
    }

    /// Set the visible time range by start/end timestamps.
    /// Adjusts bar spacing and scroll offset to fit the range.
    #[unsafe(no_mangle)]
    pub extern "C" fn chart_time_scale_set_visible_range(
        chart: *mut Chart,
        start_time: i64,
        end_time: i64,
    ) -> bool {
        let chart = unsafe {
            assert!(!chart.is_null());
            &mut *chart
        };
        if chart.state.data.bars.is_empty() {
            return false;
        }
        // Find bar indices for start and end times
        let start_idx = chart
            .state
            .data
            .bars
            .binary_search_by_key(&start_time, |b| b.time)
            .unwrap_or_else(|i| i);
        let end_idx = chart
            .state
            .data
            .bars
            .binary_search_by_key(&end_time, |b| b.time)
            .unwrap_or_else(|i| i);
        let visible_bars = (end_idx as f32 - start_idx as f32).max(1.0);
        let plot_width = chart.state.layout.plot_area.width;
        chart.state.time_scale.bar_spacing = (plot_width / visible_bars).clamp(2.0, 50.0);
        let scroll = chart
            .state
            .time_scale
            .scroll_offset_for_first(start_idx as f32, plot_width);
        chart.state.time_scale.scroll_offset = scroll;
        chart.state.time_scale.clamp_scroll();
        chart.state.update_price_scale();
        chart
            .state
            .pending_mask
            .set_global(crate::invalidation::InvalidationLevel::Light);
        true
    }

    /// Get the visible logical range (first_bar_index, last_bar_index) as f64.
    #[unsafe(no_mangle)]
    pub extern "C" fn chart_time_scale_get_visible_logical_range(
        chart: *const Chart,
        out_first: *mut f64,
        out_last: *mut f64,
    ) -> bool {
        let chart = unsafe {
            assert!(!chart.is_null());
            &*chart
        };
        let plot_width = chart.state.layout.plot_area.width;
        let first = chart.state.time_scale.first_visible_index(plot_width) as f64;
        let last = chart.state.time_scale.last_visible_index(plot_width) as f64;
        unsafe {
            if !out_first.is_null() {
                *out_first = first;
            }
            if !out_last.is_null() {
                *out_last = last;
            }
        }
        true
    }

    /// Set the visible logical range by first/last bar indices.
    #[unsafe(no_mangle)]
    pub extern "C" fn chart_time_scale_set_visible_logical_range(
        chart: *mut Chart,
        first: f64,
        last: f64,
    ) -> bool {
        let chart = unsafe {
            assert!(!chart.is_null());
            &mut *chart
        };
        let visible_bars = (last - first).max(1.0) as f32;
        let plot_width = chart.state.layout.plot_area.width;
        chart.state.time_scale.bar_spacing = (plot_width / visible_bars).clamp(2.0, 50.0);
        let scroll = chart
            .state
            .time_scale
            .scroll_offset_for_first(first as f32, plot_width);
        chart.state.time_scale.scroll_offset = scroll;
        chart.state.time_scale.clamp_scroll();
        chart.state.update_price_scale();
        chart
            .state
            .pending_mask
            .set_global(crate::invalidation::InvalidationLevel::Light);
        true
    }

    /// Reset time scale to default (fit content).
    #[unsafe(no_mangle)]
    pub extern "C" fn chart_time_scale_reset(chart: *mut Chart) -> bool {
        let chart = unsafe {
            assert!(!chart.is_null());
            &mut *chart
        };
        chart.state.fit_content()
    }

    /// Get the time scale width in logical pixels.
    #[unsafe(no_mangle)]
    pub extern "C" fn chart_time_scale_width(chart: *const Chart) -> f32 {
        let chart = unsafe {
            assert!(!chart.is_null());
            &*chart
        };
        chart.state.layout.plot_area.width
    }

    /// Get the time scale height in logical pixels.
    #[unsafe(no_mangle)]
    pub extern "C" fn chart_time_scale_height(chart: *const Chart) -> f32 {
        let chart = unsafe {
            assert!(!chart.is_null());
            &*chart
        };
        chart.state.layout.plot_area.height
    }

    // ===================================================================
    // ISeriesApi — seriesType, dataByIndex
    // ===================================================================

    /// Get the series type as u8: 0=Ohlc, 1=Candlestick, 2=Line, 3=Area, 4=Baseline, 5=Histogram
    #[unsafe(no_mangle)]
    pub extern "C" fn chart_series_type(chart: *const Chart, series_id: u32) -> i32 {
        let chart = unsafe {
            assert!(!chart.is_null());
            &*chart
        };
        if let Some(s) = chart.state.series.get(series_id) {
            match s.series_type {
                crate::series::SeriesType::Ohlc => 0,
                crate::series::SeriesType::Candlestick => 1,
                crate::series::SeriesType::Line => 2,
                crate::series::SeriesType::Area => 3,
                crate::series::SeriesType::Baseline => 4,
                crate::series::SeriesType::Histogram => 5,
            }
        } else {
            -1 // not found
        }
    }

    /// Get bar data by index for primary data. Returns OHLC values.
    /// Returns true if index is valid.
    #[unsafe(no_mangle)]
    pub extern "C" fn chart_data_by_index(
        chart: *const Chart,
        index: u32,
        out_time: *mut i64,
        out_open: *mut f64,
        out_high: *mut f64,
        out_low: *mut f64,
        out_close: *mut f64,
    ) -> bool {
        let chart = unsafe {
            assert!(!chart.is_null());
            &*chart
        };
        if let Some(bar) = chart.state.data.bars.get(index as usize) {
            unsafe {
                if !out_time.is_null() {
                    *out_time = bar.time;
                }
                if !out_open.is_null() {
                    *out_open = bar.open;
                }
                if !out_high.is_null() {
                    *out_high = bar.high;
                }
                if !out_low.is_null() {
                    *out_low = bar.low;
                }
                if !out_close.is_null() {
                    *out_close = bar.close;
                }
            }
            true
        } else {
            false
        }
    }

    // ===================================================================
    // IPriceScaleApi
    // ===================================================================

    /// Get the price scale mode: 0 = Normal, 1 = Logarithmic.
    #[unsafe(no_mangle)]
    pub extern "C" fn chart_price_scale_get_mode(chart: *const Chart, pane_index: u32) -> u8 {
        let chart = unsafe {
            assert!(!chart.is_null());
            &*chart
        };
        if let Some(pane) = chart.state.panes.get(pane_index as usize) {
            match pane.price_scale.mode {
                crate::price_scale::PriceScaleMode::Normal => 0,
                crate::price_scale::PriceScaleMode::Logarithmic => 1,
            }
        } else {
            0
        }
    }

    /// Set the price scale mode: 0 = Normal, 1 = Logarithmic.
    #[unsafe(no_mangle)]
    pub extern "C" fn chart_price_scale_set_mode(
        chart: *mut Chart,
        pane_index: u32,
        mode: u8,
    ) -> bool {
        let chart = unsafe {
            assert!(!chart.is_null());
            &mut *chart
        };
        if let Some(pane) = chart.state.panes.get_mut(pane_index as usize) {
            pane.price_scale.mode = match mode {
                1 => crate::price_scale::PriceScaleMode::Logarithmic,
                _ => crate::price_scale::PriceScaleMode::Normal,
            };
            chart
                .state
                .pending_mask
                .set_global(crate::invalidation::InvalidationLevel::Full);
            true
        } else {
            false
        }
    }

    /// Get the current visible price range for a pane.
    #[unsafe(no_mangle)]
    pub extern "C" fn chart_price_scale_get_range(
        chart: *const Chart,
        pane_index: u32,
        out_min: *mut f64,
        out_max: *mut f64,
    ) -> bool {
        let chart = unsafe {
            assert!(!chart.is_null());
            &*chart
        };
        if let Some(pane) = chart.state.panes.get(pane_index as usize) {
            unsafe {
                if !out_min.is_null() {
                    *out_min = pane.price_scale.min_price;
                }
                if !out_max.is_null() {
                    *out_max = pane.price_scale.max_price;
                }
            }
            true
        } else {
            false
        }
    }

    // ===================================================================
    // Localization — format helpers
    // ===================================================================

    /// Format a price using the chart's localization options.
    /// Caller must free with chart_free_string.
    #[unsafe(no_mangle)]
    pub extern "C" fn chart_format_price(
        chart: *const Chart,
        price: f64,
    ) -> *mut std::os::raw::c_char {
        let chart = unsafe {
            assert!(!chart.is_null());
            &*chart
        };
        let formatted = chart.state.options.price_scale.format.format(price);
        std::ffi::CString::new(formatted)
            .unwrap_or_default()
            .into_raw()
    }

    /// Format a timestamp using the chart's localization date format.
    /// Caller must free with chart_free_string.
    #[unsafe(no_mangle)]
    pub extern "C" fn chart_format_date(
        chart: *const Chart,
        timestamp: i64,
    ) -> *mut std::os::raw::c_char {
        let chart = unsafe {
            assert!(!chart.is_null());
            &*chart
        };
        let formatted = crate::formatters::format_date_custom(
            timestamp,
            &chart.state.options.localization.date_format,
        );
        std::ffi::CString::new(formatted)
            .unwrap_or_default()
            .into_raw()
    }

    /// Format a timestamp using the chart's localization time format.
    /// Caller must free with chart_free_string.
    #[unsafe(no_mangle)]
    pub extern "C" fn chart_format_time(
        chart: *const Chart,
        timestamp: i64,
    ) -> *mut std::os::raw::c_char {
        let chart = unsafe {
            assert!(!chart.is_null());
            &*chart
        };
        let formatted = crate::formatters::format_time_custom(
            timestamp,
            &chart.state.options.localization.time_format,
        );
        std::ffi::CString::new(formatted)
            .unwrap_or_default()
            .into_raw()
    }
}
