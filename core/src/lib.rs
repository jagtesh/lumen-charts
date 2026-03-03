#[cfg(target_arch = "wasm32")]
pub mod backend_canvas2d;
#[cfg(feature = "femtovg-backend")]
pub mod backend_femtovg;
pub mod backend_vello;
pub mod chart_model;
pub mod chart_options;
pub mod chart_renderer;
pub mod chart_state;
pub mod data_layer;
pub mod draw_backend;
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
// C-ABI / Foreign Function Interface (Shared across Native and WASM)
// ---------------------------------------------------------------------------
use std::ffi::c_void;

use crate::backend_vello::VelloBackend;
use crate::chart_model::ChartData;
use crate::chart_renderer::{render_bottom_scene, render_crosshair_scene};
use crate::chart_state::ChartState;

use vello::wgpu;
use vello::{AaConfig, Renderer as VelloRenderer, RendererOptions, Scene};

use wgpu::rwh;

/// Identifies the type of native view handle passed to `chart_create`.
///
/// The caller creates a view (a renderable rectangle within their window)
/// and passes its handle via this enum. wgpu automatically selects the
/// best GPU backend (Metal, DX12, Vulkan) — the caller never needs to
/// think about GPU APIs.
///
/// | Kind    | view_handle        | display_handle          |
/// |---------|--------------------|-------------------------|
/// | Metal   | CAMetalLayer*      | NULL                    |
/// | Win32   | child HWND         | NULL                    |
/// | X11     | X11 Window (cast)  | Display*                |
/// | Wayland | wl_surface*        | wl_display*             |
#[repr(C)]
pub enum ChartViewKind {
    Metal = 0,
    Win32 = 1,
    X11 = 2,
    Wayland = 3,
}

#[repr(C)]
pub struct ChartEventParam {
    pub time: i64,
    pub logical: f64,
    pub point_x: f32,
    pub point_y: f32,
    pub price: f64,
}

pub type ChartEventCallback = extern "C" fn(param: *const ChartEventParam, user_data: *mut c_void);

/// Callback for range change events: fires with (from, to) values.
pub type RangeChangeCallback = extern "C" fn(from: f64, to: f64, user_data: *mut c_void);

/// Callback for size change events: fires with (width, height).
pub type SizeChangeCallback = extern "C" fn(width: f32, height: f32, user_data: *mut c_void);

/// Chart handle containing GPU context and chart state.
/// Fields are public to allow platform-specific initialization (e.g., WASM canvas).
pub struct Chart {
    pub state: ChartState,
    pub backend: VelloBackend,
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

    // Event subscriptions for ITimeScaleApi
    pub visible_time_range_cb: Option<(RangeChangeCallback, *mut c_void)>,
    pub visible_logical_range_cb: Option<(RangeChangeCallback, *mut c_void)>,
    pub size_change_cb: Option<(SizeChangeCallback, *mut c_void)>,

    // Previous state for change detection
    pub prev_visible_time_range: Option<(i64, i64)>,
    pub prev_visible_logical_range: Option<(f32, f32)>,
    pub prev_chart_size: Option<(f32, f32)>,
}

impl Chart {
    /// Create a Chart from a pre-created wgpu instance and surface.
    ///
    /// This is the Rust-native entry point — Rust consumers (e.g. the winit
    /// demo) create their own `wgpu::Instance` and `wgpu::Surface`, then
    /// pass both here. The library never creates wgpu resources on its own.
    pub fn new_from_surface(
        instance: wgpu::Instance,
        surface: wgpu::Surface<'static>,
        width: u32,
        height: u32,
        scale_factor: f64,
    ) -> Self {
        Self::init_with_surface(instance, surface, width, height, scale_factor)
    }

    /// Shared initialization: given a wgpu instance and surface, set up the
    /// adapter, device, Vello renderer, and return a ready-to-render Chart.
    fn init_with_surface(
        instance: wgpu::Instance,
        surface: wgpu::Surface<'static>,
        width: u32,
        height: u32,
        scale_factor: f64,
    ) -> Self {
        let data = ChartData { bars: Vec::new() };
        let state = ChartState::new(data, width as f32, height as f32, scale_factor);

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

        Chart {
            state,
            backend: backend_vello::VelloBackend::new(),
            device,
            queue,
            surface,
            surface_config,
            vello_renderer,
            cached_bottom_scene: None,
            click_cb: None,
            crosshair_move_cb: None,
            dbl_click_cb: None,
            visible_time_range_cb: None,
            visible_logical_range_cb: None,
            size_change_cb: None,
            prev_visible_time_range: None,
            prev_visible_logical_range: None,
            prev_chart_size: None,
        }
    }
}

// ----- Lifecycle -----

/// Create a chart attached to a native view.
///
/// `view_kind`       — identifies the type of native view handle.
/// `view_handle`     — the renderable rectangle (CAMetalLayer*, child HWND, etc.)
/// `display_handle`  — display connection for X11/Wayland (NULL for Metal/Win32).
/// `width`, `height` — logical dimensions of the view in points.
/// `scale_factor`    — HiDPI scale (e.g. 2.0 for Retina).
///
/// The chart fills the entire view. Use `chart_resize()` when the view changes size.
#[cfg(not(target_arch = "wasm32"))]
#[cfg_attr(not(target_arch = "wasm32"), no_mangle)]
pub extern "C" fn chart_create(
    view_kind: ChartViewKind,
    view_handle: *mut c_void,
    display_handle: *mut c_void,
    width: u32,
    height: u32,
    scale_factor: f64,
) -> *mut Chart {
    env_logger::try_init().ok();

    let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
        backends: wgpu::Backends::all(),
        ..Default::default()
    });

    let surface = unsafe {
        let target = match view_kind {
            ChartViewKind::Metal => wgpu::SurfaceTargetUnsafe::CoreAnimationLayer(view_handle),
            ChartViewKind::Win32 => {
                let raw_window = rwh::RawWindowHandle::Win32(rwh::Win32WindowHandle::new(
                    std::num::NonZeroIsize::new(view_handle as isize)
                        .expect("view_handle (HWND) must not be null"),
                ));
                let raw_display = rwh::RawDisplayHandle::Windows(rwh::WindowsDisplayHandle::new());
                wgpu::SurfaceTargetUnsafe::RawHandle {
                    raw_display_handle: raw_display,
                    raw_window_handle: raw_window,
                }
            }
            ChartViewKind::X11 => {
                let raw_window =
                    rwh::RawWindowHandle::Xlib(rwh::XlibWindowHandle::new(view_handle as u64));
                let raw_display = rwh::RawDisplayHandle::Xlib(rwh::XlibDisplayHandle::new(
                    std::ptr::NonNull::new(display_handle),
                    0,
                ));
                wgpu::SurfaceTargetUnsafe::RawHandle {
                    raw_display_handle: raw_display,
                    raw_window_handle: raw_window,
                }
            }
            ChartViewKind::Wayland => {
                let raw_window = rwh::RawWindowHandle::Wayland(rwh::WaylandWindowHandle::new(
                    std::ptr::NonNull::new(view_handle)
                        .expect("view_handle (wl_surface) must not be null"),
                ));
                let raw_display = rwh::RawDisplayHandle::Wayland(rwh::WaylandDisplayHandle::new(
                    std::ptr::NonNull::new(display_handle)
                        .expect("display_handle (wl_display) must not be null"),
                ));
                wgpu::SurfaceTargetUnsafe::RawHandle {
                    raw_display_handle: raw_display,
                    raw_window_handle: raw_window,
                }
            }
        };

        instance
            .create_surface_unsafe(target)
            .expect("Failed to create wgpu surface from native view handle")
    };

    let chart = Chart::init_with_surface(instance, surface, width, height, scale_factor);
    Box::into_raw(Box::new(chart))
}

/// Internal render implementation shared by both explicit and conditional paths.
fn render_internal(chart: &mut Chart, level: crate::invalidation::InvalidationLevel) {
    chart.backend.reset();

    if level.needs_bottom_scene() {
        // Light or Full — rebuild the bottom scene via backend
        let mut bottom_backend = VelloBackend::new();
        render_bottom_scene(&mut bottom_backend, &chart.state);
        let bottom_scene = bottom_backend.scene;
        chart.backend.scene_mut().append(&bottom_scene, None);
        chart.cached_bottom_scene = Some(bottom_scene);
        chart.state.bottom_render_count += 1;
    } else if let Some(ref cached) = chart.cached_bottom_scene {
        // Cursor only — reuse cached bottom scene
        chart.backend.scene_mut().append(cached, None);
    } else {
        // No cache yet — must do full render
        let mut bottom_backend = VelloBackend::new();
        render_bottom_scene(&mut bottom_backend, &chart.state);
        let bottom_scene = bottom_backend.scene;
        chart.backend.scene_mut().append(&bottom_scene, None);
        chart.cached_bottom_scene = Some(bottom_scene);
        chart.state.bottom_render_count += 1;
    }

    // Always render crosshair on top
    render_crosshair_scene(&mut chart.backend, &chart.state);
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
            chart.backend.scene(),
            &surface_texture,
            &render_params,
        )
        .expect("Vello render failed");

    surface_texture.present();

    // Fire any range/size change callbacks
    fire_change_events(chart);
}

/// Render the chart unconditionally. Call this after explicit state mutations
/// to ensure the display is updated immediately.
#[cfg_attr(not(target_arch = "wasm32"), no_mangle)]
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
#[cfg_attr(not(target_arch = "wasm32"), no_mangle)]
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

#[cfg_attr(not(target_arch = "wasm32"), no_mangle)]
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

#[cfg_attr(not(target_arch = "wasm32"), no_mangle)]
pub extern "C" fn chart_destroy(chart: *mut Chart) {
    if !chart.is_null() {
        unsafe {
            drop(Box::from_raw(chart));
        }
    }
}

// ----- Interaction C-ABI (all return bool: needs_redraw) -----

#[cfg_attr(not(target_arch = "wasm32"), no_mangle)]
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

#[cfg_attr(not(target_arch = "wasm32"), no_mangle)]
pub extern "C" fn chart_pointer_down(chart: *mut Chart, x: f32, y: f32, button: u8) -> bool {
    let chart = unsafe {
        assert!(!chart.is_null());
        &mut *chart
    };
    chart.state.pointer_down(x, y, button)
}

#[cfg_attr(not(target_arch = "wasm32"), no_mangle)]
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

#[cfg_attr(not(target_arch = "wasm32"), no_mangle)]
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

#[cfg_attr(not(target_arch = "wasm32"), no_mangle)]
pub extern "C" fn chart_unsubscribe_click(chart: *mut Chart) {
    let chart = unsafe {
        assert!(!chart.is_null());
        &mut *chart
    };
    chart.click_cb = None;
}

#[cfg_attr(not(target_arch = "wasm32"), no_mangle)]
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

#[cfg_attr(not(target_arch = "wasm32"), no_mangle)]
pub extern "C" fn chart_unsubscribe_dbl_click(chart: *mut Chart) {
    let chart = unsafe {
        assert!(!chart.is_null());
        &mut *chart
    };
    chart.dbl_click_cb = None;
}

// ----- Touch events -----

#[cfg_attr(not(target_arch = "wasm32"), no_mangle)]
pub extern "C" fn chart_touch_start(chart: *mut Chart, id: u32, x: f32, y: f32) -> bool {
    let chart = unsafe {
        assert!(!chart.is_null());
        &mut *chart
    };
    chart
        .state
        .touch_start(crate::chart_state::TouchPoint { id, x, y })
}

#[cfg_attr(not(target_arch = "wasm32"), no_mangle)]
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
#[cfg_attr(not(target_arch = "wasm32"), no_mangle)]
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
#[cfg_attr(not(target_arch = "wasm32"), no_mangle)]
pub extern "C" fn chart_touch_tick(chart: *mut Chart) {
    let chart = unsafe {
        assert!(!chart.is_null());
        &mut *chart
    };
    chart.state.touch_tick();
}

#[cfg_attr(not(target_arch = "wasm32"), no_mangle)]
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

#[cfg_attr(not(target_arch = "wasm32"), no_mangle)]
pub extern "C" fn chart_unsubscribe_crosshair_move(chart: *mut Chart) {
    let chart = unsafe {
        assert!(!chart.is_null());
        &mut *chart
    };
    chart.crosshair_move_cb = None;
}

#[cfg_attr(not(target_arch = "wasm32"), no_mangle)]
pub extern "C" fn chart_pointer_leave(chart: *mut Chart) -> bool {
    let chart = unsafe {
        assert!(!chart.is_null());
        &mut *chart
    };
    chart.state.pointer_leave()
}

#[cfg_attr(not(target_arch = "wasm32"), no_mangle)]
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

#[cfg_attr(not(target_arch = "wasm32"), no_mangle)]
pub extern "C" fn chart_clear_crosshair_position(chart: *mut Chart) -> bool {
    let chart = unsafe {
        assert!(!chart.is_null());
        &mut *chart
    };
    chart.state.clear_crosshair_position()
}

#[cfg_attr(not(target_arch = "wasm32"), no_mangle)]
pub extern "C" fn chart_scroll(chart: *mut Chart, delta_x: f32, delta_y: f32) -> bool {
    let chart = unsafe {
        assert!(!chart.is_null());
        &mut *chart
    };
    chart.state.scroll(delta_x, delta_y)
}

#[cfg_attr(not(target_arch = "wasm32"), no_mangle)]
pub extern "C" fn chart_zoom(chart: *mut Chart, factor: f32, center_x: f32) -> bool {
    let chart = unsafe {
        assert!(!chart.is_null());
        &mut *chart
    };
    chart.state.zoom(factor, center_x)
}

#[cfg_attr(not(target_arch = "wasm32"), no_mangle)]
pub extern "C" fn chart_pinch(chart: *mut Chart, scale: f32, center_x: f32, center_y: f32) -> bool {
    let chart = unsafe {
        assert!(!chart.is_null());
        &mut *chart
    };
    chart.state.pinch(scale, center_x, center_y)
}

#[cfg_attr(not(target_arch = "wasm32"), no_mangle)]
pub extern "C" fn chart_fit_content(chart: *mut Chart) -> bool {
    let chart = unsafe {
        assert!(!chart.is_null());
        &mut *chart
    };
    chart.state.fit_content()
}

#[cfg_attr(not(target_arch = "wasm32"), no_mangle)]
pub extern "C" fn chart_key_down(chart: *mut Chart, key_code: u32) -> bool {
    let chart = unsafe {
        assert!(!chart.is_null());
        &mut *chart
    };
    let key = crate::chart_state::ChartKey::from_code(key_code);
    chart.state.key_down(key)
}

#[cfg_attr(not(target_arch = "wasm32"), no_mangle)]
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
#[cfg_attr(not(target_arch = "wasm32"), no_mangle)]
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
#[cfg_attr(not(target_arch = "wasm32"), no_mangle)]
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
#[cfg_attr(not(target_arch = "wasm32"), no_mangle)]
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
#[cfg_attr(not(target_arch = "wasm32"), no_mangle)]
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
#[cfg_attr(not(target_arch = "wasm32"), no_mangle)]
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
            Some(crate::draw_backend::Color([
                bytes[0] as f32 / 255.0,
                bytes[1] as f32 / 255.0,
                bytes[2] as f32 / 255.0,
                bytes[3] as f32 / 255.0,
            ]))
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
#[cfg_attr(not(target_arch = "wasm32"), no_mangle)]
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
#[cfg_attr(not(target_arch = "wasm32"), no_mangle)]
pub extern "C" fn chart_bar_count(chart: *mut Chart) -> u32 {
    let chart = unsafe {
        assert!(!chart.is_null());
        &*chart
    };
    chart.state.bar_count() as u32
}

/// Set the active series type. 0=OHLC, 1=Candlestick, 2=Line.
#[cfg_attr(not(target_arch = "wasm32"), no_mangle)]
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
#[cfg_attr(not(target_arch = "wasm32"), unsafe(no_mangle))]
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
#[cfg_attr(not(target_arch = "wasm32"), unsafe(no_mangle))]
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

#[cfg_attr(not(target_arch = "wasm32"), unsafe(no_mangle))]
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
            let json_str = unsafe { std::ffi::CStr::from_ptr(options_json_cstr) }.to_string_lossy();
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

#[cfg_attr(not(target_arch = "wasm32"), no_mangle)]
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

#[cfg_attr(not(target_arch = "wasm32"), no_mangle)]
pub extern "C" fn chart_price_to_coordinate(chart: *mut Chart, price: f64) -> f32 {
    let chart = unsafe {
        assert!(!chart.is_null());
        &mut *chart
    };
    chart.state.panes[0]
        .price_scale
        .price_to_y(price, &chart.state.panes[0].layout_rect)
}

#[cfg_attr(not(target_arch = "wasm32"), no_mangle)]
pub extern "C" fn chart_coordinate_to_price(chart: *mut Chart, y: f32) -> f64 {
    let chart = unsafe {
        assert!(!chart.is_null());
        &mut *chart
    };
    chart.state.panes[0]
        .price_scale
        .y_to_price(y, &chart.state.panes[0].layout_rect)
}

#[cfg_attr(not(target_arch = "wasm32"), no_mangle)]
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

#[cfg_attr(not(target_arch = "wasm32"), no_mangle)]
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

#[cfg_attr(not(target_arch = "wasm32"), no_mangle)]
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

#[cfg_attr(not(target_arch = "wasm32"), no_mangle)]
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

#[cfg_attr(not(target_arch = "wasm32"), no_mangle)]
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

#[cfg_attr(not(target_arch = "wasm32"), no_mangle)]
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

#[cfg_attr(not(target_arch = "wasm32"), no_mangle)]
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

#[cfg_attr(not(target_arch = "wasm32"), no_mangle)]
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

#[cfg_attr(not(target_arch = "wasm32"), no_mangle)]
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
#[cfg_attr(not(target_arch = "wasm32"), unsafe(no_mangle))]
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
#[cfg_attr(not(target_arch = "wasm32"), unsafe(no_mangle))]
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
#[cfg_attr(not(target_arch = "wasm32"), unsafe(no_mangle))]
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
            Some(crate::draw_backend::Color([
                bytes[0] as f32 / 255.0,
                bytes[1] as f32 / 255.0,
                bytes[2] as f32 / 255.0,
                bytes[3] as f32 / 255.0,
            ]))
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
#[cfg_attr(not(target_arch = "wasm32"), unsafe(no_mangle))]
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
#[cfg_attr(not(target_arch = "wasm32"), unsafe(no_mangle))]
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
#[cfg_attr(not(target_arch = "wasm32"), unsafe(no_mangle))]
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
#[cfg_attr(not(target_arch = "wasm32"), unsafe(no_mangle))]
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
#[cfg_attr(not(target_arch = "wasm32"), unsafe(no_mangle))]
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
#[cfg_attr(not(target_arch = "wasm32"), unsafe(no_mangle))]
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
            crate::draw_backend::Color([r, g, b, a])
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
#[cfg_attr(not(target_arch = "wasm32"), unsafe(no_mangle))]
pub extern "C" fn chart_remove_series(chart: *mut Chart, series_id: u32) -> bool {
    let chart = unsafe {
        assert!(!chart.is_null());
        &mut *chart
    };
    chart.state.remove_series(series_id)
}

/// Get the number of additional series on the chart.
#[cfg_attr(not(target_arch = "wasm32"), unsafe(no_mangle))]
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
#[cfg_attr(not(target_arch = "wasm32"), unsafe(no_mangle))]
pub extern "C" fn chart_add_pane(chart: *mut Chart, height_stretch: f32) -> u32 {
    let chart = unsafe {
        assert!(!chart.is_null());
        &mut *chart
    };
    chart.state.add_pane(height_stretch)
}

/// Remove a pane by ID. Returns true if removed.
/// Pane 0 (main) cannot be removed. Orphaned series move to pane 0.
#[cfg_attr(not(target_arch = "wasm32"), unsafe(no_mangle))]
pub extern "C" fn chart_remove_pane(chart: *mut Chart, pane_id: u32) -> bool {
    let chart = unsafe {
        assert!(!chart.is_null());
        &mut *chart
    };
    chart.state.remove_pane(pane_id)
}

/// Move a series to a specific pane (by pane ID).
/// Returns true if both the series and pane were found.
#[cfg_attr(not(target_arch = "wasm32"), unsafe(no_mangle))]
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
#[cfg_attr(not(target_arch = "wasm32"), unsafe(no_mangle))]
pub extern "C" fn chart_pane_count(chart: *const Chart) -> u32 {
    let chart = unsafe {
        assert!(!chart.is_null());
        &*chart
    };
    chart.state.panes.len() as u32
}

/// Swap two panes by their IDs.
#[cfg_attr(not(target_arch = "wasm32"), unsafe(no_mangle))]
pub extern "C" fn chart_swap_panes(chart: *mut Chart, pane_id_a: u32, pane_id_b: u32) -> bool {
    let chart = unsafe {
        assert!(!chart.is_null());
        &mut *chart
    };
    chart.state.swap_panes(pane_id_a, pane_id_b)
}

/// Get the layout rect of a pane by ID.
/// Returns true if pane exists, writing x/y/width/height to the out pointers.
#[cfg_attr(not(target_arch = "wasm32"), unsafe(no_mangle))]
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
#[cfg_attr(not(target_arch = "wasm32"), unsafe(no_mangle))]
pub extern "C" fn chart_get_options(chart: *const Chart) -> *mut std::os::raw::c_char {
    let chart = unsafe {
        assert!(!chart.is_null());
        &*chart
    };
    let json = serde_json::to_string(&chart.state.options).unwrap_or_default();
    std::ffi::CString::new(json).unwrap_or_default().into_raw()
}

/// Free a string returned by chart_get_options.
#[cfg_attr(not(target_arch = "wasm32"), unsafe(no_mangle))]
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
#[cfg_attr(not(target_arch = "wasm32"), unsafe(no_mangle))]
pub extern "C" fn chart_time_scale_scroll_to_position(chart: *mut Chart, position: f32) -> bool {
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
#[cfg_attr(not(target_arch = "wasm32"), unsafe(no_mangle))]
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
#[cfg_attr(not(target_arch = "wasm32"), unsafe(no_mangle))]
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
#[cfg_attr(not(target_arch = "wasm32"), unsafe(no_mangle))]
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
#[cfg_attr(not(target_arch = "wasm32"), unsafe(no_mangle))]
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
#[cfg_attr(not(target_arch = "wasm32"), unsafe(no_mangle))]
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
#[cfg_attr(not(target_arch = "wasm32"), unsafe(no_mangle))]
pub extern "C" fn chart_time_scale_reset(chart: *mut Chart) -> bool {
    let chart = unsafe {
        assert!(!chart.is_null());
        &mut *chart
    };
    chart.state.fit_content()
}

/// Get the time scale width in logical pixels.
#[cfg_attr(not(target_arch = "wasm32"), unsafe(no_mangle))]
pub extern "C" fn chart_time_scale_width(chart: *const Chart) -> f32 {
    let chart = unsafe {
        assert!(!chart.is_null());
        &*chart
    };
    chart.state.layout.plot_area.width
}

/// Get the time scale height in logical pixels.
#[cfg_attr(not(target_arch = "wasm32"), unsafe(no_mangle))]
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
#[cfg_attr(not(target_arch = "wasm32"), unsafe(no_mangle))]
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
#[cfg_attr(not(target_arch = "wasm32"), unsafe(no_mangle))]
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
#[cfg_attr(not(target_arch = "wasm32"), unsafe(no_mangle))]
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
#[cfg_attr(not(target_arch = "wasm32"), unsafe(no_mangle))]
pub extern "C" fn chart_price_scale_set_mode(chart: *mut Chart, pane_index: u32, mode: u8) -> bool {
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
#[cfg_attr(not(target_arch = "wasm32"), unsafe(no_mangle))]
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
#[cfg_attr(not(target_arch = "wasm32"), unsafe(no_mangle))]
pub extern "C" fn chart_format_price(chart: *const Chart, price: f64) -> *mut std::os::raw::c_char {
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
#[cfg_attr(not(target_arch = "wasm32"), unsafe(no_mangle))]
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
#[cfg_attr(not(target_arch = "wasm32"), unsafe(no_mangle))]
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

// ----- ISeriesApi: Markers -----

/// Set markers on a series from a JSON array string.
/// JSON format: [{"time":1704153600,"shape":"arrowUp","position":"belowBar","color":[0.15,0.65,0.6,1.0],"size":8,"text":"Buy"}, ...]
/// Valid shapes: "arrowUp", "arrowDown", "circle", "square"
/// Valid positions: "aboveBar", "belowBar", "atPrice"
/// Caller must free json_cstr.
#[cfg_attr(not(target_arch = "wasm32"), unsafe(no_mangle))]
pub extern "C" fn chart_series_set_markers(
    chart: *mut Chart,
    _series_id: u32,
    markers_json: *const std::os::raw::c_char,
) -> bool {
    let chart = unsafe {
        assert!(!chart.is_null());
        &mut *chart
    };

    let json_str = unsafe {
        assert!(!markers_json.is_null());
        std::ffi::CStr::from_ptr(markers_json)
    };
    let json_str = match json_str.to_str() {
        Ok(s) => s,
        Err(_) => return false,
    };

    let parsed: Result<Vec<serde_json::Value>, _> = serde_json::from_str(json_str);
    let items = match parsed {
        Ok(v) => v,
        Err(_) => return false,
    };

    let mut markers = Vec::with_capacity(items.len());
    for item in &items {
        let time = item.get("time").and_then(|v| v.as_i64()).unwrap_or(0);
        let shape_str = item
            .get("shape")
            .and_then(|v| v.as_str())
            .unwrap_or("circle");
        let pos_str = item
            .get("position")
            .and_then(|v| v.as_str())
            .unwrap_or("aboveBar");

        let shape = match shape_str {
            "arrowUp" => crate::overlays::MarkerShape::ArrowUp,
            "arrowDown" => crate::overlays::MarkerShape::ArrowDown,
            "square" => crate::overlays::MarkerShape::Square,
            _ => crate::overlays::MarkerShape::Circle,
        };
        let position = match pos_str {
            "belowBar" => crate::overlays::MarkerPosition::BelowBar,
            "atPrice" => crate::overlays::MarkerPosition::AtPrice,
            _ => crate::overlays::MarkerPosition::AboveBar,
        };

        let mut marker = crate::overlays::SeriesMarker::new(time, shape, position);

        if let Some(color) = item.get("color").and_then(|v| v.as_array()) {
            if color.len() == 4 {
                marker.color = crate::draw_backend::Color([
                    color[0].as_f64().unwrap_or(0.0) as f32,
                    color[1].as_f64().unwrap_or(0.0) as f32,
                    color[2].as_f64().unwrap_or(0.0) as f32,
                    color[3].as_f64().unwrap_or(1.0) as f32,
                ]);
            }
        }
        if let Some(size) = item.get("size").and_then(|v| v.as_f64()) {
            marker.size = size as f32;
        }
        if let Some(text) = item.get("text").and_then(|v| v.as_str()) {
            marker.text = text.to_string();
        }

        markers.push(marker);
    }

    chart.state.overlays.set_markers(markers);
    chart
        .state
        .pending_mask
        .set_global(crate::invalidation::InvalidationLevel::Full);
    true
}

/// Get markers for a series as a JSON string.
/// Caller must free with chart_free_string.
#[cfg_attr(not(target_arch = "wasm32"), unsafe(no_mangle))]
pub extern "C" fn chart_series_markers(
    chart: *const Chart,
    _series_id: u32,
) -> *mut std::os::raw::c_char {
    let chart = unsafe {
        assert!(!chart.is_null());
        &*chart
    };

    let mut arr = Vec::new();
    for m in &chart.state.overlays.markers {
        let shape_str = match m.shape {
            crate::overlays::MarkerShape::ArrowUp => "arrowUp",
            crate::overlays::MarkerShape::ArrowDown => "arrowDown",
            crate::overlays::MarkerShape::Circle => "circle",
            crate::overlays::MarkerShape::Square => "square",
        };
        let pos_str = match m.position {
            crate::overlays::MarkerPosition::AboveBar => "aboveBar",
            crate::overlays::MarkerPosition::BelowBar => "belowBar",
            crate::overlays::MarkerPosition::AtPrice => "atPrice",
        };
        arr.push(serde_json::json!({
            "time": m.time,
            "shape": shape_str,
            "position": pos_str,
            "color": m.color,
            "size": m.size,
            "text": m.text,
        }));
    }

    let json = serde_json::to_string(&arr).unwrap_or_else(|_| "[]".to_string());
    std::ffi::CString::new(json).unwrap_or_default().into_raw()
}

// ----- ISeriesApi: options() -----

/// Get the current options for a series as a JSON string.
/// Caller must free with chart_free_string.
#[cfg_attr(not(target_arch = "wasm32"), unsafe(no_mangle))]
pub extern "C" fn chart_series_get_options(
    chart: *const Chart,
    series_id: u32,
) -> *mut std::os::raw::c_char {
    let chart = unsafe {
        assert!(!chart.is_null());
        &*chart
    };

    let json = if let Some(series) = chart.state.series.get(series_id) {
        match series.series_type {
            crate::series::SeriesType::Ohlc | crate::series::SeriesType::Candlestick => {
                serde_json::to_string(&series.candlestick_options).unwrap_or_default()
            }
            crate::series::SeriesType::Line => {
                serde_json::to_string(&series.line_options).unwrap_or_default()
            }
            crate::series::SeriesType::Area => {
                serde_json::to_string(&series.area_options).unwrap_or_default()
            }
            crate::series::SeriesType::Histogram => {
                serde_json::to_string(&series.histogram_options).unwrap_or_default()
            }
            crate::series::SeriesType::Baseline => {
                serde_json::to_string(&series.baseline_options).unwrap_or_default()
            }
        }
    } else {
        "{}".to_string()
    };

    std::ffi::CString::new(json).unwrap_or_default().into_raw()
}

// ----- ISeriesApi: barsInLogicalRange -----

/// Returns the number of bars in a series that fall within the given logical index range.
#[cfg_attr(not(target_arch = "wasm32"), unsafe(no_mangle))]
pub extern "C" fn chart_series_bars_in_logical_range(
    chart: *const Chart,
    series_id: u32,
    from: f32,
    to: f32,
) -> u32 {
    let chart = unsafe {
        assert!(!chart.is_null());
        &*chart
    };

    if let Some(series) = chart.state.series.get(series_id) {
        let from_idx = from.floor().max(0.0) as usize;
        let to_idx = to.ceil().max(0.0) as usize;
        let data_len = series.data.len();
        if from_idx >= data_len {
            return 0;
        }
        let to_idx = to_idx.min(data_len);
        (to_idx - from_idx) as u32
    } else {
        0
    }
}

// ----- IPriceScaleApi: applyOptions / width -----

/// Apply options to the price scale via JSON.
#[cfg_attr(not(target_arch = "wasm32"), unsafe(no_mangle))]
pub extern "C" fn chart_price_scale_apply_options(
    chart: *mut Chart,
    json_cstr: *const std::os::raw::c_char,
) -> bool {
    let chart = unsafe {
        assert!(!chart.is_null());
        &mut *chart
    };

    let json_str = unsafe {
        assert!(!json_cstr.is_null());
        std::ffi::CStr::from_ptr(json_cstr)
    };
    let json_str = match json_str.to_str() {
        Ok(s) => s,
        Err(_) => return false,
    };

    // Price scale options are part of the chart options
    let wrapper = format!("{{\"rightPriceScale\":{}}}", json_str);
    if chart.state.options.apply_json(&wrapper) {
        chart.state.update_price_scale();
        chart
            .state
            .pending_mask
            .set_global(crate::invalidation::InvalidationLevel::Full);
        true
    } else {
        false
    }
}

/// Get the width of the price scale in pixels.
#[cfg_attr(not(target_arch = "wasm32"), unsafe(no_mangle))]
pub extern "C" fn chart_price_scale_width(chart: *const Chart) -> f32 {
    let chart = unsafe {
        assert!(!chart.is_null());
        &*chart
    };
    chart.state.layout.margins.right
}

/// Get the current price scale options/state as a JSON string.
/// Returns: {"mode":"normal"|"logarithmic","minPrice":..,"maxPrice":..,"width":..}
/// Caller must free with chart_free_string.
#[cfg_attr(not(target_arch = "wasm32"), unsafe(no_mangle))]
pub extern "C" fn chart_price_scale_get_options(chart: *const Chart) -> *mut std::os::raw::c_char {
    let chart = unsafe {
        assert!(!chart.is_null());
        &*chart
    };
    let ps = &chart.state.panes[0].price_scale;
    let mode_str = match ps.mode {
        crate::price_scale::PriceScaleMode::Normal => "normal",
        crate::price_scale::PriceScaleMode::Logarithmic => "logarithmic",
    };
    let json = serde_json::json!({
        "mode": mode_str,
        "minPrice": ps.min_price,
        "maxPrice": ps.max_price,
        "width": chart.state.layout.margins.right,
    });
    std::ffi::CString::new(json.to_string())
        .unwrap_or_default()
        .into_raw()
}

// ----- ITimeScaleApi: applyOptions -----

/// Apply options to the time scale via JSON.
#[cfg_attr(not(target_arch = "wasm32"), unsafe(no_mangle))]
pub extern "C" fn chart_time_scale_apply_options(
    chart: *mut Chart,
    json_cstr: *const std::os::raw::c_char,
) -> bool {
    let chart = unsafe {
        assert!(!chart.is_null());
        &mut *chart
    };

    let json_str = unsafe {
        assert!(!json_cstr.is_null());
        std::ffi::CStr::from_ptr(json_cstr)
    };
    let json_str = match json_str.to_str() {
        Ok(s) => s,
        Err(_) => return false,
    };

    // Time scale options are part of the chart options
    let wrapper = format!("{{\"timeScale\":{}}}", json_str);
    if chart.state.options.apply_json(&wrapper) {
        chart
            .state
            .pending_mask
            .set_global(crate::invalidation::InvalidationLevel::Full);
        true
    } else {
        false
    }
}

// ----- Crosshair seriesData -----

/// Get the values of all series at the current crosshair time.
/// Fills out_series_ids and out_values with paired data.
/// Returns the number of entries written.
#[cfg_attr(not(target_arch = "wasm32"), unsafe(no_mangle))]
pub extern "C" fn chart_crosshair_get_series_data(
    chart: *const Chart,
    out_series_ids: *mut u32,
    out_values: *mut f64,
    max_count: u32,
) -> u32 {
    let chart = unsafe {
        assert!(!chart.is_null());
        &*chart
    };

    let bar_index = match chart.state.crosshair.bar_index {
        Some(i) => i,
        None => return 0,
    };

    // Get the time from the primary data at this bar index
    let crosshair_time = chart
        .state
        .data
        .bars
        .get(bar_index)
        .map(|b| b.time)
        .unwrap_or(0);
    if crosshair_time == 0 {
        return 0;
    }

    let mut count = 0u32;
    for series in &chart.state.series.series {
        if count >= max_count {
            break;
        }

        // Binary search for the crosshair time in this series' data
        let value = match &series.data {
            crate::series::SeriesData::Ohlc(bars) => bars
                .binary_search_by_key(&crosshair_time, |b| b.time)
                .ok()
                .map(|i| bars[i].close),
            crate::series::SeriesData::Line(pts) => pts
                .binary_search_by_key(&crosshair_time, |p| p.time)
                .ok()
                .map(|i| pts[i].value),
            crate::series::SeriesData::Histogram(pts) => pts
                .binary_search_by_key(&crosshair_time, |p| p.time)
                .ok()
                .map(|i| pts[i].value),
        };

        if let Some(val) = value {
            unsafe {
                *out_series_ids.add(count as usize) = series.id;
                *out_values.add(count as usize) = val;
            }
            count += 1;
        }
    }

    count
}

// ----- ITimeScaleApi: Event Subscriptions -----

/// Subscribe to visible time range changes.
#[cfg_attr(not(target_arch = "wasm32"), unsafe(no_mangle))]
pub extern "C" fn chart_time_scale_subscribe_visible_time_range_change(
    chart: *mut Chart,
    callback: RangeChangeCallback,
    user_data: *mut c_void,
) {
    let chart = unsafe {
        assert!(!chart.is_null());
        &mut *chart
    };
    chart.visible_time_range_cb = Some((callback, user_data));
}

/// Unsubscribe from visible time range changes.
#[cfg_attr(not(target_arch = "wasm32"), unsafe(no_mangle))]
pub extern "C" fn chart_time_scale_unsubscribe_visible_time_range_change(chart: *mut Chart) {
    let chart = unsafe {
        assert!(!chart.is_null());
        &mut *chart
    };
    chart.visible_time_range_cb = None;
}

/// Subscribe to visible logical range changes.
#[cfg_attr(not(target_arch = "wasm32"), unsafe(no_mangle))]
pub extern "C" fn chart_time_scale_subscribe_visible_logical_range_change(
    chart: *mut Chart,
    callback: RangeChangeCallback,
    user_data: *mut c_void,
) {
    let chart = unsafe {
        assert!(!chart.is_null());
        &mut *chart
    };
    chart.visible_logical_range_cb = Some((callback, user_data));
}

/// Unsubscribe from visible logical range changes.
#[cfg_attr(not(target_arch = "wasm32"), unsafe(no_mangle))]
pub extern "C" fn chart_time_scale_unsubscribe_visible_logical_range_change(chart: *mut Chart) {
    let chart = unsafe {
        assert!(!chart.is_null());
        &mut *chart
    };
    chart.visible_logical_range_cb = None;
}

/// Subscribe to time scale size changes.
#[cfg_attr(not(target_arch = "wasm32"), unsafe(no_mangle))]
pub extern "C" fn chart_time_scale_subscribe_size_change(
    chart: *mut Chart,
    callback: SizeChangeCallback,
    user_data: *mut c_void,
) {
    let chart = unsafe {
        assert!(!chart.is_null());
        &mut *chart
    };
    chart.size_change_cb = Some((callback, user_data));
}

/// Unsubscribe from time scale size changes.
#[cfg_attr(not(target_arch = "wasm32"), unsafe(no_mangle))]
pub extern "C" fn chart_time_scale_unsubscribe_size_change(chart: *mut Chart) {
    let chart = unsafe {
        assert!(!chart.is_null());
        &mut *chart
    };
    chart.size_change_cb = None;
}

// ----- Fire event subscriptions (called internally after render) -----

/// Check for range/size changes and fire subscribed callbacks.
/// Should be called after each render.
fn fire_change_events(chart: &mut Chart) {
    let plot_width = chart.state.layout.plot_area.width;

    // Check visible time range
    if !chart.state.data.bars.is_empty() {
        let (first, last) = chart.state.time_scale.visible_range(plot_width);
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
        let current = (start_time, end_time);
        if chart.prev_visible_time_range != Some(current) {
            chart.prev_visible_time_range = Some(current);
            if let Some((cb, ud)) = chart.visible_time_range_cb {
                cb(start_time as f64, end_time as f64, ud);
            }
        }
    }

    // Check visible logical range
    let first_logical = chart.state.time_scale.first_visible_index(plot_width) as f32;
    let last_logical = chart.state.time_scale.last_visible_index(plot_width) as f32;
    let current_logical = (first_logical, last_logical);
    if chart.prev_visible_logical_range != Some(current_logical) {
        chart.prev_visible_logical_range = Some(current_logical);
        if let Some((cb, ud)) = chart.visible_logical_range_cb {
            cb(first_logical as f64, last_logical as f64, ud);
        }
    }

    // Check chart size
    let current_size = (chart.state.layout.width, chart.state.layout.height);
    if chart.prev_chart_size != Some(current_size) {
        chart.prev_chart_size = Some(current_size);
        if let Some((cb, ud)) = chart.size_change_cb {
            cb(current_size.0, current_size.1, ud);
        }
    }
}
