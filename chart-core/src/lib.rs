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

    /// Opaque chart handle passed via C-ABI
    pub struct Chart {
        state: ChartState,
        scene: Scene,
        device: wgpu::Device,
        queue: wgpu::Queue,
        surface: wgpu::Surface<'static>,
        surface_config: wgpu::SurfaceConfiguration,
        vello_renderer: VelloRenderer,
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
        chart.state.pointer_move(x, y)
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
        chart.state.pointer_up(x, y, button)
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
            _ => return false,
        };
        true
    }
}
