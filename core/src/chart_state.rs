use crate::chart_model::{ChartData, ChartLayout, OhlcBar};
use crate::chart_options::ChartOptions;
use crate::invalidation::{InvalidateMask, InvalidationLevel};
use crate::overlays::Overlays;
use crate::price_scale::PriceScale;
use crate::series::{SeriesCollection, SeriesType};
use crate::time_scale::TimeScale;

/// Crosshair position state
#[derive(Debug, Clone, Default)]
pub struct Crosshair {
    /// Whether the crosshair is currently visible
    pub visible: bool,
    /// Pointer position in logical coordinates
    pub x: f32,
    pub y: f32,
    /// Snapped bar index (if pointer is over a bar)
    pub bar_index: Option<usize>,
    /// Price at the y coordinate
    pub price: Option<f64>,
}

/// Which zone the pointer interacted with
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum HitZone {
    PlotArea,
    YAxis,
    XAxis,
    None,
}

/// Drag state for panning
#[derive(Debug, Clone, Default)]
pub struct DragState {
    pub active: bool,
    pub zone: Option<HitZone>,
    pub start_x: f32,
    pub start_y: f32,
    pub last_x: f32,
    pub last_y: f32,
    /// Total distance moved (for click vs drag detection)
    pub total_distance: f32,
}

impl Default for HitZone {
    fn default() -> Self {
        HitZone::None
    }
}

/// Click event data passed to callback
#[derive(Debug, Clone)]
pub struct ClickEvent {
    pub x: f32,
    pub y: f32,
    pub bar_index: Option<usize>,
    pub price: Option<f64>,
}

/// Crosshair move event data
#[derive(Debug, Clone)]
pub struct CrosshairMoveEvent {
    pub x: f32,
    pub y: f32,
    pub bar_index: Option<usize>,
    pub price: Option<f64>,
    pub visible: bool,
}

/// Aggregated events from a single interaction call.
/// The host can inspect this to fire callbacks.
#[derive(Debug, Clone, Default)]
pub struct InteractionEvents {
    pub click: Option<ClickEvent>,
    pub dbl_click: Option<ClickEvent>,
    pub crosshair_move: Option<CrosshairMoveEvent>,
}

/// Keyboard keys
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ChartKey {
    ArrowLeft,
    ArrowRight,
    ArrowUp,
    ArrowDown,
    Plus,
    Minus,
    Home,
    End,
    Unknown,
}

impl ChartKey {
    pub fn from_code(code: u32) -> Self {
        match code {
            37 => ChartKey::ArrowLeft,
            39 => ChartKey::ArrowRight,
            38 => ChartKey::ArrowUp,
            40 => ChartKey::ArrowDown,
            187 | 61 => ChartKey::Plus,   // = / + key
            189 | 173 => ChartKey::Minus, // - key
            36 => ChartKey::Home,
            35 => ChartKey::End,
            _ => ChartKey::Unknown,
        }
    }
}

/// Click threshold — if pointer moves less than this many pixels, it's a click
const CLICK_DISTANCE_THRESHOLD: f32 = 5.0;
/// Double-click detection: max interval between two clicks (milliseconds).
/// We use a frame-count approximation since we don't have real timers.
const DBL_CLICK_MAX_FRAMES: u32 = 20; // ~20 frames ≈ 333ms at 60fps

/// A single pane containing its own price scale and bounding box.
///
/// # v5 Design: Index-based identity
///
/// In LWC v4/Lumen-original, panes had unique `u32` IDs assigned by a counter.
/// v5 switched to an index-based model where a pane's position in the `Vec<PaneState>`
/// *is* its identity (`paneIndex`). This is simpler and matches the v5 API semantics
/// where `addSeries(def, opts, paneIndex)` and `getPane().paneIndex()` use indices.
///
/// No backwards-compatibility shim is needed — we directly adopt v5's model.
#[derive(Debug, Clone)]
pub struct PaneState {
    pub price_scale: PriceScale,
    pub height_stretch: f32,
    pub layout_rect: crate::chart_model::Rect,
}

impl PaneState {
    pub fn new(price_scale: PriceScale, layout_rect: crate::chart_model::Rect) -> Self {
        Self {
            price_scale,
            height_stretch: 1.0,
            layout_rect,
        }
    }
}

/// A single touch point from the platform layer.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TouchPoint {
    pub id: u32,
    pub x: f32,
    pub y: f32,
}

/// What gesture the touch system has recognized.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum TouchGesture {
    /// No gesture yet (too early to tell)
    None,
    /// Single-finger pan
    Pan,
    /// Two-finger pinch/zoom
    Pinch,
    /// Quick tap (touch down + up within distance threshold)
    Tap,
    /// Long press (held without moving)
    LongPress,
}

/// Internal state for touch gesture recognition.
#[derive(Debug, Clone)]
pub struct TouchState {
    /// Currently active touch points (max 2 for pinch)
    pub touches: Vec<TouchPoint>,
    /// What gesture has been recognized
    pub gesture: TouchGesture,
    /// Accumulated distance for the first finger (for tap vs drag detection)
    pub total_distance: f32,
    /// Last known pinch distance (for delta calculation)
    pub last_pinch_distance: f32,
    /// Center X of the pinch gesture
    pub pinch_center_x: f32,
    /// Frame counter since first touch down (for long-press detection)
    pub frames_since_down: u32,
}

impl Default for TouchState {
    fn default() -> Self {
        Self {
            touches: Vec::new(),
            gesture: TouchGesture::None,
            total_distance: 0.0,
            last_pinch_distance: 0.0,
            pinch_center_x: 0.0,
            frames_since_down: 0,
        }
    }
}

impl TouchState {
    /// Distance between two touch points
    fn pinch_distance(a: &TouchPoint, b: &TouchPoint) -> f32 {
        let dx = a.x - b.x;
        let dy = a.y - b.y;
        (dx * dx + dy * dy).sqrt()
    }

    /// Center between two touch points
    fn pinch_center(a: &TouchPoint, b: &TouchPoint) -> (f32, f32) {
        ((a.x + b.x) / 2.0, (a.y + b.y) / 2.0)
    }
}

/// Distance threshold for tap detection (pixels)
const TOUCH_TAP_THRESHOLD: f32 = 10.0;
/// Frame threshold for long-press (frames at 60fps, ~500ms)
const LONG_PRESS_FRAMES: u32 = 30;

use std::collections::HashMap;

/// Full mutable chart state — owns the model+view state
pub struct ChartState {
    pub data: ChartData,
    pub time_scale: TimeScale,
    pub panes: Vec<PaneState>,
    pub layout: ChartLayout,
    pub crosshair: Crosshair,
    pub drag: DragState,
    pub options: ChartOptions,
    pub overlays: Overlays,
    pub active_series_type: SeriesType,
    pub series: SeriesCollection,

    // Click/dbl-click detection
    last_click_x: f32,
    last_click_y: f32,
    frames_since_last_click: u32,
    click_pending: bool,

    // Y-axis drag state (for price scale zoom)
    y_axis_drag_start_range: Option<(f64, f64)>,
    // X-axis drag state (for time scale zoom)
    x_axis_drag_start_spacing: Option<f32>,
    /// Which pane is currently being interacted with (set on pointer_down)
    pub active_pane: usize,

    /// Pending events from the last interaction call
    pub pending_events: InteractionEvents,

    /// Pending invalidation mask — accumulated between renders
    pub pending_mask: InvalidateMask,

    // v5: Panes use index-based identity. No ID counter needed.

    // --- Render counters (for testing and profiling) ---
    /// Number of full bottom-scene renders (background + grid + series + axes)
    pub bottom_render_count: u64,
    /// Number of crosshair-only renders
    pub crosshair_render_count: u64,
    /// Number of renders skipped due to None mask
    pub skipped_render_count: u64,

    /// Touch gesture state
    pub touch: TouchState,

    /// Global time index map: timestamp → bar index (mirrors LWC v5 _pointDataByTimePoint).
    /// Used by series renderers to align data points with the shared time axis.
    pub time_index_map: HashMap<i64, usize>,
    /// Sorted unified time points (parallel to time_index_map).
    /// Enables index → time lookup for visible_range to time range conversion.
    pub time_points: Vec<i64>,
}

impl ChartState {
    pub fn new(data: ChartData, width: f32, height: f32, scale_factor: f64) -> Self {
        Self::with_options(data, width, height, scale_factor, ChartOptions::default())
    }

    pub fn with_options(
        data: ChartData,
        width: f32,
        height: f32,
        scale_factor: f64,
        options: ChartOptions,
    ) -> Self {
        let layout = ChartLayout::new(width, height, scale_factor);
        let time_scale = TimeScale::new(data.bars.len(), layout.plot_area.width);
        let price_scale = PriceScale::from_data(&data.bars);
        let panes = vec![PaneState::new(price_scale, layout.plot_area)];

        let mut state = ChartState {
            data,
            time_scale,
            panes,
            layout,
            crosshair: Crosshair::default(),
            drag: DragState::default(),
            options,
            overlays: Overlays::new(),
            active_series_type: SeriesType::default(),
            series: SeriesCollection::new(),
            last_click_x: 0.0,
            last_click_y: 0.0,
            frames_since_last_click: DBL_CLICK_MAX_FRAMES + 1,
            click_pending: false,
            y_axis_drag_start_range: None,
            x_axis_drag_start_spacing: None,
            active_pane: 0,
            pending_events: InteractionEvents::default(),
            pending_mask: InvalidateMask::full(), // first render needs full paint
            // v5: Panes use index-based identity. No ID counter needed.
            bottom_render_count: 0,
            crosshair_render_count: 0,
            skipped_render_count: 0,
            touch: TouchState::default(),
            time_index_map: HashMap::new(),
            time_points: Vec::new(),
        };
        state.rebuild_time_index();
        state.update_panes_layout();
        state
    }

    /// Recalculate layout after resize
    pub fn resize(&mut self, width: f32, height: f32, scale_factor: f64) {
        self.layout = ChartLayout::new(width, height, scale_factor);
        self.update_panes_layout();
        self.update_price_scale();
        self.pending_mask.set_global(InvalidationLevel::Full);
    }

    /// Distributes total plot_area height among the panes based on stretch
    pub fn update_panes_layout(&mut self) {
        let total_stretch: f32 = self.panes.iter().map(|p| p.height_stretch).sum();
        if total_stretch == 0.0 {
            return;
        }

        // Between panes, we could leave a 1 or 2 pixel gap. Let's do 1px for now,
        // or just no gap since we can draw a separator.
        let num_panes = self.panes.len() as f32;
        let gap = 1.0;
        let available_height = self.layout.plot_area.height - (gap * (num_panes - 1.0).max(0.0));

        let mut current_y = self.layout.plot_area.y;

        for pane in &mut self.panes {
            let pane_height = available_height * (pane.height_stretch / total_stretch);
            pane.layout_rect = crate::chart_model::Rect {
                x: self.layout.plot_area.x,
                y: current_y,
                width: self.layout.plot_area.width,
                height: pane_height,
            };
            current_y += pane_height + gap;
        }
    }

    /// Update price scale to fit visible data for all panes
    pub fn update_price_scale(&mut self) {
        let (first, last) = self.time_scale.visible_range(self.layout.plot_area.width);
        if first >= last || last > self.time_points.len() {
            return;
        }

        // Get the visible time range from the unified time points
        let start_time = self.time_points[first];
        let end_time = self.time_points[last - 1];

        // We need to calculate auto-scale bounds for EACH pane based on the series assigned to it
        for (i, pane) in self.panes.iter_mut().enumerate() {
            // Find all series belonging to this pane
            let mut has_series = false;
            let mut min_val = f64::INFINITY;
            let mut max_val = f64::NEG_INFINITY;

            // Optional: If this is pane 0 and no series exist, it should still scale to the main OHLC data layer for backward compatibility
            if i == 0 {
                // Iterate main OHLC bars in the visible time range
                for bar in &self.data.bars {
                    if bar.time >= start_time && bar.time <= end_time {
                        min_val = min_val.min(bar.low);
                        max_val = max_val.max(bar.high);
                        has_series = true;
                    }
                }
            }

            for series in self.series.series.iter() {
                if series.pane_index == i && series.visible {
                    let series_min_max = match &series.data {
                        crate::series::SeriesData::Line(pts) => {
                            let mut s_min = f64::INFINITY;
                            let mut s_max = f64::NEG_INFINITY;

                            for pt in pts.iter() {
                                if pt.time >= start_time && pt.time <= end_time {
                                    s_min = s_min.min(pt.value);
                                    s_max = s_max.max(pt.value);
                                }
                            }
                            if s_min <= s_max {
                                Some((s_min, s_max))
                            } else {
                                None
                            }
                        }
                        crate::series::SeriesData::Ohlc(bars) => {
                            let mut s_min = f64::INFINITY;
                            let mut s_max = f64::NEG_INFINITY;
                            for b in bars.iter() {
                                if b.time >= start_time && b.time <= end_time {
                                    s_min = s_min.min(b.low);
                                    s_max = s_max.max(b.high);
                                }
                            }
                            if s_min <= s_max {
                                Some((s_min, s_max))
                            } else {
                                None
                            }
                        }
                        crate::series::SeriesData::Histogram(pts) => {
                            let mut s_min = f64::INFINITY;
                            let mut s_max = f64::NEG_INFINITY;
                            for pt in pts.iter() {
                                if pt.time >= start_time && pt.time <= end_time {
                                    // Histogram scale typically includes 0
                                    s_min = s_min.min(0.0).min(pt.value);
                                    // Make sure max is at least above 0 or slightly above min to avoid flat scale
                                    s_max = s_max.max(0.0).max(pt.value);
                                }
                            }
                            if s_min <= s_max {
                                Some((s_min, s_max))
                            } else {
                                None
                            }
                        }
                    };

                    if let Some((s_min, s_max)) = series_min_max {
                        min_val = min_val.min(s_min);
                        max_val = max_val.max(s_max);
                        has_series = true;
                    }
                }
            }

            if has_series {
                // Add margins like PriceScale::from_data does (5% margin)
                let range = max_val - min_val;
                let margin = if range == 0.0 { 1.0 } else { range * 0.05 };
                pane.price_scale.min_price = min_val - margin;
                pane.price_scale.max_price = max_val + margin;
            }
        }
    }

    /// Add a new pane to the chart. Returns the pane *index* (v5 model).
    /// `height_stretch` controls how much vertical space this pane gets relative to others.
    pub fn add_pane(&mut self, height_stretch: f32) -> u32 {
        let price_scale = PriceScale::from_data(&[]);
        let mut pane = PaneState::new(
            price_scale,
            crate::chart_model::Rect {
                x: 0.0,
                y: 0.0,
                width: 0.0,
                height: 0.0,
            },
        );
        pane.height_stretch = height_stretch;
        self.panes.push(pane);
        let index = (self.panes.len() - 1) as u32;
        self.update_panes_layout();
        self.update_price_scale();
        self.pending_mask.set_global(InvalidationLevel::Full);
        index
    }

    /// Remove a pane by index (v5 model). Returns true if the pane was removed.
    /// Pane 0 (the main pane) cannot be removed.
    /// Any series assigned to this pane are moved back to pane 0.
    pub fn remove_pane(&mut self, pane_index: u32) -> bool {
        let idx = pane_index as usize;
        if idx == 0 || idx >= self.panes.len() {
            return false;
        }
        self.panes.remove(idx);
        // Move orphaned series back to pane 0, adjust indices for panes that shifted
        for series in &mut self.series.series {
            if series.pane_index == idx {
                series.pane_index = 0;
            } else if series.pane_index > idx {
                series.pane_index -= 1;
            }
        }
        self.update_panes_layout();
        self.update_price_scale();
        self.pending_mask.set_global(InvalidationLevel::Full);
        true
    }

    /// Move a series to a specific pane by index (v5 model).
    /// Returns true if both the series and pane were found.
    pub fn move_series_to_pane(&mut self, series_id: u32, pane_index: u32) -> bool {
        let idx = pane_index as usize;
        if idx >= self.panes.len() {
            return false;
        }
        if let Some(series) = self.series.get_mut(series_id) {
            series.pane_index = idx;
            self.update_price_scale();
            self.pending_mask.set_global(InvalidationLevel::Full);
            return true;
        }
        false
    }

    /// Swap two panes by their indices (v5 model). Returns true if both were valid.
    pub fn swap_panes(&mut self, index_a: u32, index_b: u32) -> bool {
        let a = index_a as usize;
        let b = index_b as usize;
        if a >= self.panes.len() || b >= self.panes.len() || a == b {
            return false;
        }
        self.panes.swap(a, b);
        // Update series pane_index references
        for series in &mut self.series.series {
            if series.pane_index == a {
                series.pane_index = b;
            } else if series.pane_index == b {
                series.pane_index = a;
            }
        }
        self.update_panes_layout();
        self.update_price_scale();
        self.pending_mask.set_global(InvalidationLevel::Full);
        true
    }

    /// Get the layout rect (x, y, width, height) for a pane by index (v5 model).
    /// Returns None if the index is out of bounds.
    pub fn pane_size(&self, pane_index: u32) -> Option<(f32, f32, f32, f32)> {
        self.panes.get(pane_index as usize).map(|p| {
            (
                p.layout_rect.x,
                p.layout_rect.y,
                p.layout_rect.width,
                p.layout_rect.height,
            )
        })
    }

    /// Determine which pane index a given y coordinate falls within.
    /// Returns the pane index, or 0 if no match (defaults to main pane).
    pub fn pane_index_for_point(&self, y: f32) -> usize {
        for (i, pane) in self.panes.iter().enumerate() {
            let r = &pane.layout_rect;
            if y >= r.y && y < r.y + r.height {
                return i;
            }
        }
        0
    }

    /// Determine which zone a point is in
    fn hit_zone(&self, x: f32, y: f32) -> HitZone {
        if self.layout.plot_area_contains(x, y) {
            HitZone::PlotArea
        } else if self.layout.y_axis_contains(x, y) {
            HitZone::YAxis
        } else if self.layout.x_axis_contains(x, y) {
            HitZone::XAxis
        } else {
            HitZone::None
        }
    }

    /// Advance internal frame counter (call once per render).
    /// This is needed for dbl-click timing.
    pub fn tick(&mut self) {
        if self.frames_since_last_click <= DBL_CLICK_MAX_FRAMES {
            self.frames_since_last_click += 1;
        }
    }

    // --- Interaction handlers (all return true if redraw needed) ---

    /// Pointer moved to (x, y) in logical coordinates
    pub fn pointer_move(&mut self, x: f32, y: f32) -> bool {
        let in_plot = self.layout.plot_area_contains(x, y);
        let was_visible = self.crosshair.visible;
        self.pending_events = InteractionEvents::default();

        if in_plot {
            self.active_pane = self.pane_index_for_point(y);
            self.crosshair.visible = true;
            self.crosshair.x = x;
            self.crosshair.y = y;
            self.crosshair.bar_index = self
                .time_scale
                .x_to_nearest_index(x, &self.layout.plot_area);
            self.crosshair.price = Some(
                self.panes[self.active_pane]
                    .price_scale
                    .y_to_price(y, &self.panes[self.active_pane].layout_rect),
            );

            // Handle drag panning (plot area drag)
            if self.drag.active {
                if self.drag.zone == Some(HitZone::PlotArea) {
                    let delta_px = self.drag.last_x - x;
                    self.time_scale.scroll_by_pixels(delta_px);
                    self.update_price_scale();
                    self.pending_mask.set_global(InvalidationLevel::Light);
                } else {
                    self.pending_mask.set_global(InvalidationLevel::Cursor);
                }
                let dx = x - self.drag.last_x;
                let dy = y - self.drag.last_y;
                self.drag.total_distance += (dx * dx + dy * dy).sqrt();
                self.drag.last_x = x;
                self.drag.last_y = y;
            } else {
                self.pending_mask.set_global(InvalidationLevel::Cursor);
            }

            self.pending_events.crosshair_move = Some(CrosshairMoveEvent {
                x,
                y,
                bar_index: self.crosshair.bar_index,
                price: self.crosshair.price,
                visible: true,
            });

            true
        } else if self.drag.active {
            // Dragging outside plot area — continue the drag
            match self.drag.zone {
                Some(HitZone::PlotArea) => {
                    let delta_px = self.drag.last_x - x;
                    self.time_scale.scroll_by_pixels(delta_px);
                    self.update_price_scale();
                }
                Some(HitZone::YAxis) => {
                    // Y-axis drag: zoom price scale using cumulative delta from start
                    let delta_y = y - self.drag.start_y;
                    self.drag_price_scale(delta_y);
                }
                Some(HitZone::XAxis) => {
                    // X-axis drag: zoom time scale (expand/collapse bars like LWC)
                    let delta_x = x - self.drag.start_x;
                    self.drag_time_scale(delta_x);
                }
                _ => {}
            }
            self.pending_mask.set_global(InvalidationLevel::Light);
            let dx = x - self.drag.last_x;
            let dy = y - self.drag.last_y;
            self.drag.total_distance += (dx * dx + dy * dy).sqrt();
            self.drag.last_x = x;
            self.drag.last_y = y;
            true
        } else {
            self.crosshair.visible = false;
            self.crosshair.bar_index = None;
            self.crosshair.price = None;

            if was_visible {
                self.pending_mask.set_global(InvalidationLevel::Cursor);
                self.pending_events.crosshair_move = Some(CrosshairMoveEvent {
                    x,
                    y,
                    bar_index: None,
                    price: None,
                    visible: false,
                });
            }

            was_visible
        }
    }

    /// Pointer button pressed
    pub fn pointer_down(&mut self, x: f32, y: f32, _button: u8) -> bool {
        let zone = self.hit_zone(x, y);
        self.pending_events = InteractionEvents::default();

        if zone != HitZone::None {
            self.drag = DragState {
                active: true,
                zone: Some(zone),
                start_x: x,
                start_y: y,
                last_x: x,
                last_y: y,
                total_distance: 0.0,
            };

            // Save price scale range for Y-axis drag zoom
            if zone == HitZone::YAxis {
                self.active_pane = self.pane_index_for_point(y);
                self.y_axis_drag_start_range = Some((
                    self.panes[self.active_pane].price_scale.min_price,
                    self.panes[self.active_pane].price_scale.max_price,
                ));
            }
            // Save bar spacing for X-axis drag zoom
            if zone == HitZone::XAxis {
                self.x_axis_drag_start_spacing = Some(self.time_scale.bar_spacing);
            }
        }
        false
    }

    /// Pointer button released
    pub fn pointer_up(&mut self, x: f32, y: f32, _button: u8) -> bool {
        let was_dragging = self.drag.active;
        let drag_zone = self.drag.zone;
        let was_click = self.drag.total_distance < CLICK_DISTANCE_THRESHOLD;
        self.drag.active = false;
        self.y_axis_drag_start_range = None;
        self.x_axis_drag_start_spacing = None;
        self.pending_events = InteractionEvents::default();

        if was_dragging && was_click {
            // This was a click (not a drag)
            let click_event = ClickEvent {
                x,
                y,
                bar_index: self
                    .time_scale
                    .x_to_nearest_index(x, &self.layout.plot_area),
                price: if self.layout.plot_area_contains(x, y) {
                    let p = self.pane_index_for_point(y);
                    Some(
                        self.panes[p]
                            .price_scale
                            .y_to_price(y, &self.panes[p].layout_rect),
                    )
                } else {
                    None
                },
            };

            // Check for double-click
            if self.click_pending && self.frames_since_last_click <= DBL_CLICK_MAX_FRAMES {
                let dist =
                    ((x - self.last_click_x).powi(2) + (y - self.last_click_y).powi(2)).sqrt();
                if dist < CLICK_DISTANCE_THRESHOLD * 3.0 {
                    // Double click!
                    self.click_pending = false;
                    self.pending_events.dbl_click = Some(click_event.clone());

                    // Handle axis double-click auto-fit
                    match drag_zone {
                        Some(HitZone::YAxis) => {
                            self.update_price_scale();
                            return true;
                        }
                        Some(HitZone::XAxis) | Some(HitZone::PlotArea) => {
                            self.fit_content();
                            return true;
                        }
                        _ => {}
                    }

                    return true;
                }
            }

            // Single click — store for potential dbl-click detection
            self.last_click_x = x;
            self.last_click_y = y;
            self.frames_since_last_click = 0;
            self.click_pending = true;
            self.pending_events.click = Some(click_event);
            return true;
        }

        was_dragging
    }

    /// Pointer left the chart area
    pub fn pointer_leave(&mut self) -> bool {
        let was_visible = self.crosshair.visible;
        self.crosshair.visible = false;
        self.crosshair.bar_index = None;
        self.crosshair.price = None;
        self.drag.active = false;
        self.y_axis_drag_start_range = None;
        self.pending_events = InteractionEvents::default();

        if was_visible {
            self.pending_events.crosshair_move = Some(CrosshairMoveEvent {
                x: 0.0,
                y: 0.0,
                bar_index: None,
                price: None,
                visible: false,
            });
        }

        was_visible
    }

    /// Programmatically set crosshair position
    pub fn set_crosshair_position(&mut self, price: f64, time: i64, _series_id: u32) -> bool {
        // Find index for the given time
        let idx = match self.data.bars.binary_search_by_key(&time, |b| b.time) {
            Ok(i) => i, // Exact match
            Err(i) => {
                if i < self.data.bars.len() {
                    i
                } else {
                    self.data.bars.len().saturating_sub(1)
                }
            }
        };

        if self.data.bars.is_empty() {
            return false;
        }

        let x = self.time_scale.index_to_x(idx, &self.layout.plot_area);
        let y = self.panes[0]
            .price_scale
            .price_to_y(price, &self.panes[0].layout_rect);

        self.crosshair.x = x;
        self.crosshair.y = y;
        self.crosshair.visible = true;
        self.crosshair.price = Some(price);
        self.crosshair.bar_index = Some(idx);

        true
    }

    /// Programmatically clear crosshair position
    pub fn clear_crosshair_position(&mut self) -> bool {
        let was_visible = self.crosshair.visible;
        self.crosshair.visible = false;
        was_visible
    }

    /// Scroll (horizontal pan). delta_x > 0 scrolls right (sees older data)
    pub fn scroll(&mut self, delta_x: f32, _delta_y: f32) -> bool {
        if delta_x.abs() < 0.1 {
            return false;
        }
        self.time_scale.scroll_by_pixels(delta_x);
        self.update_price_scale();
        self.pending_mask.set_global(InvalidationLevel::Light);
        true
    }

    /// Zoom by a factor around a center x coordinate
    /// factor > 1.0 = zoom in, < 1.0 = zoom out
    pub fn zoom(&mut self, factor: f32, center_x: f32) -> bool {
        self.time_scale
            .zoom(factor, center_x, &self.layout.plot_area);
        self.update_price_scale();
        self.pending_mask.set_global(InvalidationLevel::Light);
        true
    }

    /// Pinch zoom (two-finger)
    pub fn pinch(&mut self, scale: f32, center_x: f32, _center_y: f32) -> bool {
        self.zoom(scale, center_x)
    }

    /// Fit all content
    pub fn fit_content(&mut self) -> bool {
        self.time_scale.fit_content(self.layout.plot_area.width);
        self.update_price_scale();
        self.pending_mask.set_global(InvalidationLevel::Light);
        true
    }

    /// Keyboard input. Returns true if needs redraw.
    pub fn key_down(&mut self, key: ChartKey) -> bool {
        let scroll_amount = self.time_scale.bar_spacing * 3.0; // scroll 3 bars at a time
        match key {
            ChartKey::ArrowLeft => {
                self.time_scale.scroll_by_pixels(-scroll_amount);
                self.update_price_scale();
                self.pending_mask.set_global(InvalidationLevel::Light);
                true
            }
            ChartKey::ArrowRight => {
                self.time_scale.scroll_by_pixels(scroll_amount);
                self.update_price_scale();
                self.pending_mask.set_global(InvalidationLevel::Light);
                true
            }
            ChartKey::ArrowUp | ChartKey::Plus => {
                let center = self.layout.plot_area.x + self.layout.plot_area.width / 2.0;
                self.zoom(1.2, center)
            }
            ChartKey::ArrowDown | ChartKey::Minus => {
                let center = self.layout.plot_area.x + self.layout.plot_area.width / 2.0;
                self.zoom(0.8, center)
            }
            ChartKey::Home => self.fit_content(),
            ChartKey::End => {
                // Scroll to the rightmost data (most recent)
                self.time_scale.scroll_offset = 0.0;
                self.update_price_scale();
                self.pending_mask.set_global(InvalidationLevel::Light);
                true
            }
            ChartKey::Unknown => false,
        }
    }

    /// Handle price scale drag (zooming the Y-axis).
    /// delta_y > 0 = pointer moved down = zoom out, delta_y < 0 = zoom in
    fn drag_price_scale(&mut self, delta_y: f32) {
        if let Some((orig_min, orig_max)) = self.y_axis_drag_start_range {
            let range = orig_max - orig_min;
            if range <= 0.0 {
                return;
            }
            // Scale factor: dragging 200px = 2× zoom
            let factor = 1.0 + (delta_y as f64 / 200.0);
            let factor = factor.clamp(0.1, 10.0);
            let mid = (orig_min + orig_max) / 2.0;
            let new_half = range * factor / 2.0;
            self.panes[self.active_pane].price_scale.min_price = mid - new_half;
            self.panes[self.active_pane].price_scale.max_price = mid + new_half;
        }
    }

    /// Handle time scale drag (zooming the X-axis like LWC).
    /// Dragging left = expand (stretch chart), dragging right = compress
    fn drag_time_scale(&mut self, delta_x: f32) {
        if let Some(orig_spacing) = self.x_axis_drag_start_spacing {
            // Negate: drag left (negative delta) = expand
            let factor = 1.0 + (-delta_x / 200.0);
            let factor = factor.clamp(0.1, 10.0);
            let new_spacing = (orig_spacing * factor).clamp(2.0, 50.0);
            self.time_scale.bar_spacing = new_spacing;
            self.update_price_scale();
        }
    }

    // --- Touch event handling ---

    /// A touch point started (finger down).
    /// Returns true if the state needs redrawing.
    pub fn touch_start(&mut self, point: TouchPoint) -> bool {
        if self.touch.touches.len() >= 2 {
            return false; // max 2 fingers
        }
        self.touch.touches.push(point);
        let count = self.touch.touches.len();

        if count == 1 {
            // First finger — start tracking for pan/tap/long-press
            self.touch.gesture = TouchGesture::None;
            self.touch.total_distance = 0.0;
            self.touch.frames_since_down = 0;
        } else if count == 2 {
            // Second finger — transition to pinch
            self.touch.gesture = TouchGesture::Pinch;
            let d = TouchState::pinch_distance(&self.touch.touches[0], &self.touch.touches[1]);
            let (cx, _cy) =
                TouchState::pinch_center(&self.touch.touches[0], &self.touch.touches[1]);
            self.touch.last_pinch_distance = d;
            self.touch.pinch_center_x = cx;
        }

        false // No visual change yet
    }

    /// A touch point moved (finger dragged).
    /// Returns true if the state needs redrawing.
    pub fn touch_move(&mut self, point: TouchPoint) -> bool {
        // Find and update this touch point
        let idx = self.touch.touches.iter().position(|t| t.id == point.id);
        let idx = match idx {
            Some(i) => i,
            None => return false,
        };

        let old = self.touch.touches[idx];
        let dx = point.x - old.x;
        let dy = point.y - old.y;
        self.touch.total_distance += (dx * dx + dy * dy).sqrt();
        self.touch.touches[idx] = point;

        match self.touch.touches.len() {
            1 => {
                // Single finger — recognize pan if moved enough
                if self.touch.total_distance > TOUCH_TAP_THRESHOLD {
                    self.touch.gesture = TouchGesture::Pan;
                }

                if self.touch.gesture == TouchGesture::Pan {
                    // Pan the chart: delta_x moves as scroll
                    self.time_scale.scroll_by_pixels(-dx);
                    self.update_price_scale();
                    self.pending_mask.set_global(InvalidationLevel::Light);
                    return true;
                }
                false
            }
            2 => {
                // Two fingers — pinch zoom
                self.touch.gesture = TouchGesture::Pinch;
                let d = TouchState::pinch_distance(&self.touch.touches[0], &self.touch.touches[1]);
                let (cx, _cy) =
                    TouchState::pinch_center(&self.touch.touches[0], &self.touch.touches[1]);

                if self.touch.last_pinch_distance > 0.0 {
                    let scale = d / self.touch.last_pinch_distance;
                    if (scale - 1.0).abs() > 0.001 {
                        self.pinch(scale, cx, 0.0);
                        // pinch already sets Light mask
                    }
                }

                self.touch.last_pinch_distance = d;
                self.touch.pinch_center_x = cx;
                true
            }
            _ => false,
        }
    }

    /// A touch point ended (finger up).
    /// Returns the recognized gesture for the caller to react to.
    pub fn touch_end(&mut self, point_id: u32) -> TouchGesture {
        let idx = self.touch.touches.iter().position(|t| t.id == point_id);
        if let Some(idx) = idx {
            self.touch.touches.remove(idx);
        }

        if self.touch.touches.is_empty() {
            // All fingers up — finalize gesture
            let gesture = if self.touch.total_distance < TOUCH_TAP_THRESHOLD {
                if self.touch.frames_since_down >= LONG_PRESS_FRAMES {
                    TouchGesture::LongPress
                } else {
                    TouchGesture::Tap
                }
            } else {
                self.touch.gesture
            };
            self.touch.gesture = TouchGesture::None;
            gesture
        } else {
            // Still have fingers down — might transition back to pan
            if self.touch.touches.len() == 1 {
                self.touch.gesture = TouchGesture::Pan;
            }
            self.touch.gesture
        }
    }

    /// Called each frame to advance touch timers (for long-press detection).
    pub fn touch_tick(&mut self) {
        if !self.touch.touches.is_empty() {
            self.touch.frames_since_down += 1;
        }
    }

    // --- Data management ---

    /// Replace all bar data. Resets time scale to fit new data.
    pub fn set_data(&mut self, bars: Vec<OhlcBar>) {
        self.data.bars = bars;
        self.data.bars.sort_by_key(|b| b.time);
        self.time_scale = TimeScale::new(self.data.bars.len(), self.layout.plot_area.width);
        self.rebuild_time_index();
        self.update_price_scale();
        self.pending_mask.set_global(InvalidationLevel::Light);
    }

    /// Rebuild the time→global_index map from all data sources (bars + series).
    /// Mirrors LWC v5's unified time axis: the global timeline is the sorted
    /// union of all unique timestamps across main OHLC data and every series.
    fn rebuild_time_index(&mut self) {
        // Collect all unique timestamps
        let mut times: Vec<i64> = Vec::with_capacity(self.data.bars.len());
        for bar in &self.data.bars {
            times.push(bar.time);
        }
        for series in &self.series.series {
            match &series.data {
                crate::series::SeriesData::Line(pts) => {
                    for pt in pts {
                        times.push(pt.time);
                    }
                }
                crate::series::SeriesData::Ohlc(bars) => {
                    for b in bars {
                        times.push(b.time);
                    }
                }
                crate::series::SeriesData::Histogram(pts) => {
                    for pt in pts {
                        times.push(pt.time);
                    }
                }
            }
        }

        // Sort and deduplicate
        times.sort_unstable();
        times.dedup();

        // Build the map
        self.time_index_map.clear();
        self.time_index_map.reserve(times.len());
        for (i, t) in times.iter().enumerate() {
            self.time_index_map.insert(*t, i);
        }

        // Store the sorted time points for reverse index→time lookup
        self.time_points = times;

        // Update bar_count to reflect the unified timeline size
        self.time_scale.bar_count = self.time_points.len();
    }

    /// Update or append a single bar (by timestamp).
    /// If a bar with the same timestamp exists, it is replaced.
    /// Otherwise inserted in sorted position.
    pub fn update_bar(&mut self, bar: OhlcBar) {
        match self.data.bars.binary_search_by_key(&bar.time, |b| b.time) {
            Ok(idx) => {
                self.data.bars[idx] = bar;
            }
            Err(idx) => {
                self.data.bars.insert(idx, bar);
                self.time_scale = TimeScale::new(self.data.bars.len(), self.layout.plot_area.width);
                self.rebuild_time_index();
            }
        }
        self.update_price_scale();
        self.pending_mask.set_global(InvalidationLevel::Light);
    }

    /// Remove and return the last (most recent) bar.
    pub fn pop_bar(&mut self) -> Option<OhlcBar> {
        let bar = self.data.bars.pop();
        if bar.is_some() {
            self.time_scale = TimeScale::new(self.data.bars.len(), self.layout.plot_area.width);
            self.update_price_scale();
            self.pending_mask.set_global(InvalidationLevel::Light);
        }
        bar
    }

    // --- Series management (with invalidation) ---

    /// Add a series to the chart. Returns the series ID.
    pub fn add_series(&mut self, series: crate::series::Series) -> u32 {
        let id = self.series.add(series);
        self.rebuild_time_index();
        self.update_price_scale();
        self.pending_mask.set_global(InvalidationLevel::Full);
        id
    }

    /// Remove a series by ID. Returns true if found and removed.
    pub fn remove_series(&mut self, series_id: u32) -> bool {
        let removed = self.series.remove(series_id);
        if removed {
            self.rebuild_time_index();
            self.update_price_scale();
            self.pending_mask.set_global(InvalidationLevel::Full);
        }
        removed
    }

    /// Mark that series data was mutated (called after updating series data directly).
    pub fn series_data_changed(&mut self) {
        self.rebuild_time_index();
        self.update_price_scale();
        self.pending_mask.set_global(InvalidationLevel::Light);
    }

    /// Get the number of bars.
    pub fn bar_count(&self) -> usize {
        self.data.bars.len()
    }

    // --- Options ---

    /// Apply new chart options. Returns true (always needs redraw).
    pub fn apply_options(&mut self, options: ChartOptions) -> bool {
        self.options = options;
        self.pending_mask.set_global(InvalidationLevel::Full);
        true
    }

    /// Consume the pending invalidation mask, returning it and resetting to None.
    /// Call this after rendering to clear the pending state.
    pub fn consume_mask(&mut self) -> InvalidateMask {
        let mask = self.pending_mask.clone();
        self.pending_mask.reset();
        mask
    }

    /// Get the current invalidation level without consuming it.
    pub fn invalidation_level(&self) -> InvalidationLevel {
        self.pending_mask.global_level()
    }

    /// Get a reference to current options.
    pub fn options(&self) -> &ChartOptions {
        &self.options
    }

    /// Format a price value using the configured price format.
    pub fn format_price(&self, price: f64) -> String {
        self.options.price_scale.format.format(price)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_state() -> ChartState {
        let data = ChartData {
            bars: crate::sample_data::sample_data(),
        };
        ChartState::new(data, 800.0, 500.0, 1.0)
    }

    #[test]
    fn test_pointer_move_in_plot_shows_crosshair() {
        let mut state = make_state();
        let cx = state.layout.plot_area.x + 100.0;
        let cy = state.layout.plot_area.y + 100.0;

        let needs_redraw = state.pointer_move(cx, cy);
        assert!(needs_redraw);
        assert!(state.crosshair.visible);
        assert!(state.crosshair.bar_index.is_some());
        assert!(state.crosshair.price.is_some());
    }

    #[test]
    fn test_pointer_move_outside_hides_crosshair() {
        let mut state = make_state();
        // First show crosshair
        let cx = state.layout.plot_area.x + 100.0;
        let cy = state.layout.plot_area.y + 100.0;
        state.pointer_move(cx, cy);

        // Then move outside
        let needs_redraw = state.pointer_move(-10.0, -10.0);
        assert!(needs_redraw); // was visible, now hidden
        assert!(!state.crosshair.visible);
    }

    #[test]
    fn test_drag_pan() {
        let mut state = make_state();
        let cx = state.layout.plot_area.x + 200.0;
        let cy = state.layout.plot_area.y + 200.0;

        // Start drag
        state.pointer_down(cx, cy, 0);
        assert!(state.drag.active);

        // Drag left by 50 pixels → should scroll right
        let scroll_before = state.time_scale.scroll_offset;
        state.pointer_move(cx - 50.0, cy);
        assert!(state.time_scale.scroll_offset > scroll_before);

        // Release
        state.pointer_up(cx - 50.0, cy, 0);
        assert!(!state.drag.active);
    }

    #[test]
    fn test_scroll_pans() {
        let mut state = make_state();
        let scroll_before = state.time_scale.scroll_offset;

        state.scroll(40.0, 0.0);
        assert!(state.time_scale.scroll_offset > scroll_before);
    }

    #[test]
    fn test_zoom_in() {
        let mut state = make_state();
        let spacing_before = state.time_scale.bar_spacing;
        let center = state.layout.plot_area.x + state.layout.plot_area.width / 2.0;

        state.zoom(1.5, center);
        assert!(state.time_scale.bar_spacing > spacing_before);
    }

    #[test]
    fn test_zoom_out() {
        let mut state = make_state();
        let spacing_before = state.time_scale.bar_spacing;
        let center = state.layout.plot_area.x + state.layout.plot_area.width / 2.0;

        state.zoom(0.7, center);
        assert!(state.time_scale.bar_spacing < spacing_before);
    }

    #[test]
    fn test_pointer_leave() {
        let mut state = make_state();
        let cx = state.layout.plot_area.x + 100.0;
        let cy = state.layout.plot_area.y + 100.0;
        state.pointer_move(cx, cy);

        let needs_redraw = state.pointer_leave();
        assert!(needs_redraw);
        assert!(!state.crosshair.visible);
        assert!(!state.drag.active);
    }

    #[test]
    fn test_fit_content_resets() {
        let mut state = make_state();
        state.scroll(50.0, 0.0);
        state.zoom(3.0, 400.0);

        state.fit_content();
        assert_eq!(state.time_scale.scroll_offset, 0.0);
    }

    #[test]
    fn test_resize_updates_layout() {
        let mut state = make_state();
        state.resize(1200.0, 800.0, 2.0);

        assert_eq!(state.layout.width, 1200.0);
        assert_eq!(state.layout.height, 800.0);
        assert_eq!(state.layout.scale_factor, 2.0);
    }

    // --- New Slice 2 tests ---

    #[test]
    fn test_click_detection_no_drag() {
        let mut state = make_state();
        let cx = state.layout.plot_area.x + 100.0;
        let cy = state.layout.plot_area.y + 100.0;

        state.pointer_down(cx, cy, 0);
        // Release at same position (no drag)
        let needs_redraw = state.pointer_up(cx, cy, 0);
        assert!(needs_redraw);
        assert!(state.pending_events.click.is_some());
        assert!(state.pending_events.dbl_click.is_none());
    }

    #[test]
    fn test_no_click_when_dragging() {
        let mut state = make_state();
        let cx = state.layout.plot_area.x + 200.0;
        let cy = state.layout.plot_area.y + 200.0;

        state.pointer_down(cx, cy, 0);
        // Move far enough to count as a drag
        state.pointer_move(cx + 20.0, cy);
        state.pointer_up(cx + 20.0, cy, 0);

        assert!(state.pending_events.click.is_none());
    }

    #[test]
    fn test_double_click_detection() {
        let mut state = make_state();
        let cx = state.layout.plot_area.x + 100.0;
        let cy = state.layout.plot_area.y + 100.0;

        // First click
        state.pointer_down(cx, cy, 0);
        state.pointer_up(cx, cy, 0);
        assert!(state.pending_events.click.is_some());
        assert!(state.click_pending);

        // Simulate a few frames passing
        for _ in 0..5 {
            state.tick();
        }

        // Second click at same location
        state.pointer_down(cx, cy, 0);
        state.pointer_up(cx, cy, 0);
        assert!(state.pending_events.dbl_click.is_some());
    }

    #[test]
    fn test_double_click_expired() {
        let mut state = make_state();
        let cx = state.layout.plot_area.x + 100.0;
        let cy = state.layout.plot_area.y + 100.0;

        // First click
        state.pointer_down(cx, cy, 0);
        state.pointer_up(cx, cy, 0);

        // Wait too many frames
        for _ in 0..30 {
            state.tick();
        }

        // Second click — should be a single click, not dbl
        state.pointer_down(cx, cy, 0);
        state.pointer_up(cx, cy, 0);
        assert!(state.pending_events.click.is_some());
        assert!(state.pending_events.dbl_click.is_none());
    }

    #[test]
    fn test_y_axis_drag_zooms_price() {
        let mut state = make_state();
        let ax = state.layout.plot_area.x + state.layout.plot_area.width + 10.0;
        let ay = state.layout.plot_area.y + state.layout.plot_area.height / 2.0;
        let orig_range =
            state.panes[0].price_scale.max_price - state.panes[0].price_scale.min_price;

        state.pointer_down(ax, ay, 0);
        // Drag down → should zoom out (expand range)
        state.pointer_move(ax, ay + 50.0);
        let new_range = state.panes[0].price_scale.max_price - state.panes[0].price_scale.min_price;
        assert!(
            new_range > orig_range,
            "Range should expand: {} > {}",
            new_range,
            orig_range
        );

        state.pointer_up(ax, ay + 50.0, 0);
    }

    #[test]
    fn test_x_axis_drag_zooms() {
        let mut state = make_state();
        let ax = state.layout.plot_area.x + 200.0;
        let ay = state.layout.plot_area.y + state.layout.plot_area.height + 10.0;
        let spacing_before = state.time_scale.bar_spacing;

        state.pointer_down(ax, ay, 0);
        state.pointer_move(ax - 50.0, ay); // Drag left = expand (stretch)
        assert!(
            state.time_scale.bar_spacing > spacing_before,
            "Should expand bar spacing: {} > {}",
            state.time_scale.bar_spacing,
            spacing_before
        );

        state.pointer_up(ax - 50.0, ay, 0);
    }

    #[test]
    fn test_dbl_click_y_axis_resets_price() {
        let mut state = make_state();
        let ax = state.layout.plot_area.x + state.layout.plot_area.width + 10.0;
        let ay = state.layout.plot_area.y + state.layout.plot_area.height / 2.0;

        // Manually expand price range
        state.panes[0].price_scale.min_price -= 50.0;
        state.panes[0].price_scale.max_price += 50.0;
        let expanded_range =
            state.panes[0].price_scale.max_price - state.panes[0].price_scale.min_price;

        // Double click on Y-axis
        state.pointer_down(ax, ay, 0);
        state.pointer_up(ax, ay, 0);
        for _ in 0..3 {
            state.tick();
        }
        state.pointer_down(ax, ay, 0);
        state.pointer_up(ax, ay, 0);

        // Should have reset (auto-fit to visible data)
        let reset_range =
            state.panes[0].price_scale.max_price - state.panes[0].price_scale.min_price;
        assert!(
            reset_range < expanded_range,
            "Range should shrink: {} < {}",
            reset_range,
            expanded_range
        );
    }

    #[test]
    fn test_keyboard_arrow_left() {
        let mut state = make_state();
        let scroll_before = state.time_scale.scroll_offset;

        let redraw = state.key_down(ChartKey::ArrowLeft);
        assert!(redraw);
        assert!(
            state.time_scale.scroll_offset < scroll_before,
            "Left arrow should scroll left (decrease offset)"
        );
    }

    #[test]
    fn test_keyboard_arrow_right() {
        let mut state = make_state();
        let scroll_before = state.time_scale.scroll_offset;

        let redraw = state.key_down(ChartKey::ArrowRight);
        assert!(redraw);
        assert!(
            state.time_scale.scroll_offset > scroll_before,
            "Right arrow should scroll right (increase offset)"
        );
    }

    #[test]
    fn test_keyboard_zoom() {
        let mut state = make_state();
        let spacing_before = state.time_scale.bar_spacing;

        state.key_down(ChartKey::ArrowUp);
        assert!(
            state.time_scale.bar_spacing > spacing_before,
            "Up arrow should zoom in"
        );

        let spacing_after_zoom_in = state.time_scale.bar_spacing;
        state.key_down(ChartKey::ArrowDown);
        assert!(
            state.time_scale.bar_spacing < spacing_after_zoom_in,
            "Down arrow should zoom out"
        );
    }

    #[test]
    fn test_keyboard_home_fits() {
        let mut state = make_state();
        state.scroll(100.0, 0.0);
        state.zoom(3.0, 400.0);

        state.key_down(ChartKey::Home);
        assert_eq!(state.time_scale.scroll_offset, 0.0);
    }

    #[test]
    fn test_crosshair_move_event() {
        let mut state = make_state();
        let cx = state.layout.plot_area.x + 100.0;
        let cy = state.layout.plot_area.y + 100.0;

        state.pointer_move(cx, cy);
        assert!(state.pending_events.crosshair_move.is_some());
        let evt = state.pending_events.crosshair_move.as_ref().unwrap();
        assert!(evt.visible);
        assert!(evt.bar_index.is_some());
        assert!(evt.price.is_some());
    }

    #[test]
    fn test_hit_zone_detection() {
        let state = make_state();

        // Plot area
        let px = state.layout.plot_area.x + 50.0;
        let py = state.layout.plot_area.y + 50.0;
        assert_eq!(state.hit_zone(px, py), HitZone::PlotArea);

        // Y axis (right margin)
        let yx = state.layout.plot_area.x + state.layout.plot_area.width + 10.0;
        let yy = state.layout.plot_area.y + 50.0;
        assert_eq!(state.hit_zone(yx, yy), HitZone::YAxis);

        // X axis (bottom margin)
        let xx = state.layout.plot_area.x + 50.0;
        let xy = state.layout.plot_area.y + state.layout.plot_area.height + 5.0;
        assert_eq!(state.hit_zone(xx, xy), HitZone::XAxis);

        // Outside
        assert_eq!(state.hit_zone(-10.0, -10.0), HitZone::None);
    }

    // --- Slice 3: Data management + Options tests ---

    fn make_bar(time: i64, close: f64) -> OhlcBar {
        OhlcBar {
            time,
            open: close - 1.0,
            high: close + 0.5,
            low: close - 1.5,
            close,
        }
    }

    #[test]
    fn test_set_data_replaces_bars() {
        let mut state = make_state();
        let original_count = state.bar_count();

        let new_bars = vec![make_bar(100, 50.0), make_bar(200, 60.0)];
        state.set_data(new_bars);

        assert_eq!(state.bar_count(), 2);
        assert_ne!(state.bar_count(), original_count);
        assert_eq!(state.data.bars[0].time, 100);
        assert_eq!(state.data.bars[1].time, 200);
    }

    #[test]
    fn test_set_data_sorts() {
        let mut state = make_state();
        let new_bars = vec![
            make_bar(300, 30.0),
            make_bar(100, 10.0),
            make_bar(200, 20.0),
        ];
        state.set_data(new_bars);
        assert_eq!(state.data.bars[0].time, 100);
        assert_eq!(state.data.bars[2].time, 300);
    }

    #[test]
    fn test_update_bar_append() {
        let mut state = make_state();
        let count_before = state.bar_count();
        let new_time = i64::MAX; // Way in the future
        state.update_bar(make_bar(new_time, 999.0));
        assert_eq!(state.bar_count(), count_before + 1);
    }

    #[test]
    fn test_update_bar_replace() {
        let mut state = make_state();
        let first_time = state.data.bars[0].time;
        let count_before = state.bar_count();

        state.update_bar(make_bar(first_time, 999.0));
        assert_eq!(state.bar_count(), count_before); // Same count
        assert!((state.data.bars[0].close - 999.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_pop_bar() {
        let mut state = make_state();
        let count_before = state.bar_count();

        let popped = state.pop_bar();
        assert!(popped.is_some());
        assert_eq!(state.bar_count(), count_before - 1);
    }

    #[test]
    fn test_format_price_default() {
        let state = make_state();
        let formatted = state.format_price(123.456);
        assert_eq!(formatted, "123.46"); // Default precision=2
    }

    #[test]
    fn test_apply_options() {
        let mut state = make_state();
        let mut new_opts = ChartOptions::default();
        new_opts.price_scale.format.precision = 4;
        new_opts.price_scale.format.prefix = "$".to_string();

        let needs_redraw = state.apply_options(new_opts);
        assert!(needs_redraw);
        assert_eq!(state.format_price(123.4), "$123.4000");
    }

    #[test]
    fn test_add_pane() {
        let mut state = make_state();
        assert_eq!(state.panes.len(), 1);
        let pane_id = state.add_pane(1.0);
        assert_eq!(state.panes.len(), 2);
        assert_eq!(pane_id, 1);
        // Second pane should have a non-zero layout rect
        assert!(state.panes[1].layout_rect.height > 0.0);
    }

    #[test]
    fn test_remove_pane() {
        let mut state = make_state();
        let pane_id = state.add_pane(1.0);
        assert_eq!(state.panes.len(), 2);
        let removed = state.remove_pane(pane_id);
        assert!(removed);
        assert_eq!(state.panes.len(), 1);
    }

    #[test]
    fn test_cannot_remove_main_pane() {
        let mut state = make_state();
        let removed = state.remove_pane(0);
        assert!(!removed);
        assert_eq!(state.panes.len(), 1);
    }

    #[test]
    fn test_remove_nonexistent_pane() {
        let mut state = make_state();
        let removed = state.remove_pane(999);
        assert!(!removed);
    }

    #[test]
    fn test_pane_layout_splits_height() {
        let mut state = make_state();
        let full_height = state.panes[0].layout_rect.height;
        let _pane_id = state.add_pane(1.0); // equal stretch
                                            // With 1px gap, each pane should be roughly (full_height - 1) / 2
        let half = (full_height - 1.0) / 2.0;
        assert!((state.panes[0].layout_rect.height - half).abs() < 1.0);
        assert!((state.panes[1].layout_rect.height - half).abs() < 1.0);
    }

    #[test]
    fn test_move_series_to_pane() {
        let mut state = make_state();
        let series_id = state.series.add(crate::series::Series::line(0, vec![]));
        let pane_id = state.add_pane(1.0);

        let moved = state.move_series_to_pane(series_id, pane_id);
        assert!(moved);
        assert_eq!(state.series.get(series_id).unwrap().pane_index, 1);
    }

    #[test]
    fn test_remove_pane_moves_series_back() {
        let mut state = make_state();
        let series_id = state.series.add(crate::series::Series::line(0, vec![]));
        let pane_id = state.add_pane(1.0);
        state.move_series_to_pane(series_id, pane_id);
        assert_eq!(state.series.get(series_id).unwrap().pane_index, 1);

        state.remove_pane(pane_id);
        // Series should be back in pane 0
        assert_eq!(state.series.get(series_id).unwrap().pane_index, 0);
    }

    #[test]
    fn test_unequal_height_stretch() {
        let mut state = make_state();
        let full_height = state.panes[0].layout_rect.height;
        // Pane 0 has stretch 1.0, new pane has stretch 2.0
        // Total stretch = 3.0, so pane 0 gets 1/3, pane 1 gets 2/3
        let _pane_id = state.add_pane(2.0);
        let available = full_height - 1.0; // minus 1px gap
        let expected_pane0 = available / 3.0;
        let expected_pane1 = available * 2.0 / 3.0;
        assert!((state.panes[0].layout_rect.height - expected_pane0).abs() < 1.0);
        assert!((state.panes[1].layout_rect.height - expected_pane1).abs() < 1.0);
    }

    #[test]
    fn test_sequential_pane_indices() {
        let mut state = make_state();
        // Pane 0 already exists (main pane).
        // add_pane returns the *index* of the new pane:
        let idx1 = state.add_pane(1.0);
        let idx2 = state.add_pane(1.0);
        let idx3 = state.add_pane(1.0);
        assert_eq!(idx1, 1); // [main, 1]
        assert_eq!(idx2, 2); // [main, 1, 2]
        assert_eq!(idx3, 3); // [main, 1, 2, 3]
        assert_eq!(state.panes.len(), 4);

        // Remove pane at index 2 — remaining panes shift down
        state.remove_pane(2);
        assert_eq!(state.panes.len(), 3); // [main, 1, 3(shifted to idx 2)]

        // Next add returns the new last index
        let idx4 = state.add_pane(1.0);
        assert_eq!(idx4, 3); // [main, 1, old3, new] → index 3
    }

    #[test]
    fn test_move_series_to_nonexistent_pane() {
        let mut state = make_state();
        let series_id = state.series.add(crate::series::Series::line(0, vec![]));
        let moved = state.move_series_to_pane(series_id, 999);
        assert!(!moved);
        assert_eq!(state.series.get(series_id).unwrap().pane_index, 0);
    }

    #[test]
    fn test_move_nonexistent_series_to_pane() {
        let mut state = make_state();
        let pane_id = state.add_pane(1.0);
        let moved = state.move_series_to_pane(999, pane_id);
        assert!(!moved);
    }

    #[test]
    fn test_pane_y_positions_stack_vertically() {
        let mut state = make_state();
        let _p1 = state.add_pane(1.0);
        let _p2 = state.add_pane(1.0);

        // Pane 0 starts at the top of the plot area
        assert_eq!(state.panes[0].layout_rect.y, state.layout.plot_area.y);
        // Pane 1 starts after pane 0 + gap
        let expected_y1 = state.panes[0].layout_rect.y + state.panes[0].layout_rect.height + 1.0;
        assert!((state.panes[1].layout_rect.y - expected_y1).abs() < 0.5);
        // Pane 2 starts after pane 1 + gap
        let expected_y2 = state.panes[1].layout_rect.y + state.panes[1].layout_rect.height + 1.0;
        assert!((state.panes[2].layout_rect.y - expected_y2).abs() < 0.5);
    }

    #[test]
    fn test_resize_updates_all_pane_layouts() {
        let mut state = make_state();
        let _p1 = state.add_pane(1.0);
        assert_eq!(state.panes.len(), 2);

        state.resize(1200.0, 800.0, 2.0);
        // After resize, both panes should have updated layout rects
        assert!(state.panes[0].layout_rect.width > 0.0);
        assert!(state.panes[1].layout_rect.width > 0.0);
        // They should share the plot area width
        assert_eq!(
            state.panes[0].layout_rect.width,
            state.panes[1].layout_rect.width
        );
    }

    #[test]
    fn test_pane_price_scale_isolation() {
        // When a series is in pane 1, only pane 1's price scale should reflect its range
        let mut state = make_state();
        let pane_id = state.add_pane(1.0);

        // Record pane 0's scale bounds (from main OHLC data)
        let p0_min_before = state.panes[0].price_scale.min_price;
        let p0_max_before = state.panes[0].price_scale.max_price;

        // Add a series with very different price range and move it to pane 1
        let pts = vec![
            crate::series::LineDataPoint {
                time: state.data.bars[0].time,
                value: 50000.0,
            },
            crate::series::LineDataPoint {
                time: state.data.bars[1].time,
                value: 60000.0,
            },
        ];
        let series_id = state.series.add(crate::series::Series::line(0, pts));
        state.move_series_to_pane(series_id, pane_id);

        // Pane 0 scale should be unchanged (main OHLC data stays there)
        assert!((state.panes[0].price_scale.min_price - p0_min_before).abs() < 0.01);
        assert!((state.panes[0].price_scale.max_price - p0_max_before).abs() < 0.01);

        // Pane 1 scale should reflect the 50000-60000 range
        assert!(state.panes[1].price_scale.min_price < 51000.0);
        assert!(state.panes[1].price_scale.max_price > 59000.0);
    }

    #[test]
    fn test_single_pane_layout_matches_plot_area() {
        // With only one pane, its layout_rect should match the full plot area
        let state = make_state();
        assert_eq!(state.panes.len(), 1);
        assert_eq!(state.panes[0].layout_rect.x, state.layout.plot_area.x);
        assert_eq!(state.panes[0].layout_rect.y, state.layout.plot_area.y);
        assert_eq!(
            state.panes[0].layout_rect.width,
            state.layout.plot_area.width
        );
        assert_eq!(
            state.panes[0].layout_rect.height,
            state.layout.plot_area.height
        );
    }

    #[test]
    fn test_crosshair_price_uses_pane_rect() {
        // This test verifies that pointer_move produces the correct price
        // when using a single pane, confirming the fix from the audit
        let mut state = make_state();
        let pane_rect = state.panes[0].layout_rect;

        // Move crosshair to the middle of the pane
        let mid_x = pane_rect.x + pane_rect.width / 2.0;
        let mid_y = pane_rect.y + pane_rect.height / 2.0;
        state.pointer_move(mid_x, mid_y);

        // The price should be approximately the midpoint of the price scale
        let expected_mid_price =
            (state.panes[0].price_scale.min_price + state.panes[0].price_scale.max_price) / 2.0;
        let actual_price = state.crosshair.price.unwrap();
        let tolerance =
            (state.panes[0].price_scale.max_price - state.panes[0].price_scale.min_price) * 0.1; // 10% tolerance
        assert!(
            (actual_price - expected_mid_price).abs() < tolerance,
            "Expected price ~{:.2}, got {:.2}",
            expected_mid_price,
            actual_price
        );
    }

    #[test]
    fn test_swap_panes() {
        let mut state = make_state();
        let p1 = state.add_pane(1.0);
        let p2 = state.add_pane(2.0);
        // p1 is at index 1, p2 is at index 2
        let h1_before = state.panes[1].layout_rect.height;
        let h2_before = state.panes[2].layout_rect.height;
        assert!(h2_before > h1_before); // p2 has double stretch

        let swapped = state.swap_panes(p1, p2);
        assert!(swapped);
        // After swap: pane at index 1 now has p2's stretch (2.0), index 2 has p1's stretch (1.0)
        assert!(state.panes[1].layout_rect.height > state.panes[2].layout_rect.height);
    }

    #[test]
    fn test_swap_panes_invalid() {
        let mut state = make_state();
        let result = state.swap_panes(0, 999);
        assert!(!result);
    }

    #[test]
    fn test_pane_size() {
        let state = make_state();
        let size = state.pane_size(0);
        assert!(size.is_some());
        let (x, y, w, h) = size.unwrap();
        assert_eq!(x, state.layout.plot_area.x);
        assert_eq!(y, state.layout.plot_area.y);
        assert!(w > 0.0);
        assert!(h > 0.0);
    }

    #[test]
    fn test_pane_size_nonexistent() {
        let state = make_state();
        assert!(state.pane_size(999).is_none());
    }

    #[test]
    fn test_swap_panes_updates_series_indices() {
        let mut state = make_state();
        let p1 = state.add_pane(1.0);
        let p2 = state.add_pane(1.0);
        let series_id = state.series.add(crate::series::Series::line(0, vec![]));
        state.move_series_to_pane(series_id, p1);
        assert_eq!(state.series.get(series_id).unwrap().pane_index, 1);

        state.swap_panes(p1, p2);
        // Series should now be at index 2 (where p1 moved to)
        assert_eq!(state.series.get(series_id).unwrap().pane_index, 2);
    }

    // ---- Invalidation level from interactions ----

    use crate::invalidation::InvalidationLevel;

    #[test]
    fn test_pointer_move_produces_cursor_level() {
        let mut state = make_state();
        state.consume_mask(); // clear initial Full
        let cx = state.layout.plot_area.x + 100.0;
        let cy = state.layout.plot_area.y + 100.0;
        state.pointer_move(cx, cy);
        assert_eq!(state.invalidation_level(), InvalidationLevel::Cursor);
    }

    #[test]
    fn test_drag_produces_light_level() {
        let mut state = make_state();
        state.consume_mask();
        let cx = state.layout.plot_area.x + 100.0;
        let cy = state.layout.plot_area.y + 100.0;
        // Start drag
        state.pointer_down(cx, cy, 0);
        state.consume_mask();
        // Move while dragging — this is a pan
        state.pointer_move(cx + 50.0, cy);
        assert_eq!(state.invalidation_level(), InvalidationLevel::Light);
    }

    #[test]
    fn test_scroll_produces_light_level() {
        let mut state = make_state();
        state.consume_mask();
        state.scroll(10.0, 0.0);
        assert_eq!(state.invalidation_level(), InvalidationLevel::Light);
    }

    #[test]
    fn test_zoom_produces_light_level() {
        let mut state = make_state();
        state.consume_mask();
        state.zoom(1.2, 400.0);
        assert_eq!(state.invalidation_level(), InvalidationLevel::Light);
    }

    #[test]
    fn test_resize_produces_full_level() {
        let mut state = make_state();
        state.consume_mask();
        state.resize(1024.0, 768.0, 2.0);
        assert_eq!(state.invalidation_level(), InvalidationLevel::Full);
    }

    #[test]
    fn test_add_pane_produces_full_level() {
        let mut state = make_state();
        state.consume_mask();
        state.add_pane(1.0);
        assert_eq!(state.invalidation_level(), InvalidationLevel::Full);
    }

    #[test]
    fn test_remove_pane_produces_full_level() {
        let mut state = make_state();
        let pane_id = state.add_pane(1.0);
        state.consume_mask();
        state.remove_pane(pane_id);
        assert_eq!(state.invalidation_level(), InvalidationLevel::Full);
    }

    #[test]
    fn test_set_data_produces_light_level() {
        let mut state = make_state();
        state.consume_mask();
        state.set_data(vec![crate::chart_model::OhlcBar {
            time: 1,
            open: 100.0,
            high: 110.0,
            low: 90.0,
            close: 105.0,
        }]);
        assert_eq!(state.invalidation_level(), InvalidationLevel::Light);
    }

    #[test]
    fn test_apply_options_produces_full_level() {
        let mut state = make_state();
        state.consume_mask();
        state.apply_options(ChartOptions::default());
        assert_eq!(state.invalidation_level(), InvalidationLevel::Full);
    }

    #[test]
    fn test_consume_mask_resets_to_none() {
        let mut state = make_state();
        // Initial state has Full from construction
        assert_eq!(state.invalidation_level(), InvalidationLevel::Full);
        let mask = state.consume_mask();
        assert_eq!(mask.global_level(), InvalidationLevel::Full);
        // After consume, should be None
        assert_eq!(state.invalidation_level(), InvalidationLevel::None);
    }

    #[test]
    fn test_mask_coalesces_cursor_then_light() {
        let mut state = make_state();
        state.consume_mask();
        // Mouse move → Cursor
        let cx = state.layout.plot_area.x + 100.0;
        let cy = state.layout.plot_area.y + 100.0;
        state.pointer_move(cx, cy);
        assert_eq!(state.invalidation_level(), InvalidationLevel::Cursor);
        // Scroll → Light (should upgrade from Cursor)
        state.scroll(5.0, 0.0);
        assert_eq!(state.invalidation_level(), InvalidationLevel::Light);
    }

    #[test]
    fn test_fit_content_produces_light_level() {
        let mut state = make_state();
        state.consume_mask();
        state.fit_content();
        assert_eq!(state.invalidation_level(), InvalidationLevel::Light);
    }

    #[test]
    fn test_key_down_arrow_produces_light_level() {
        let mut state = make_state();
        state.consume_mask();
        state.key_down(ChartKey::ArrowLeft);
        assert_eq!(state.invalidation_level(), InvalidationLevel::Light);
    }

    // ---- Touch event tests ----

    #[test]
    fn test_touch_tap_recognized() {
        let mut state = make_state();
        state.touch_start(TouchPoint {
            id: 1,
            x: 400.0,
            y: 250.0,
        });
        let gesture = state.touch_end(1);
        assert_eq!(gesture, TouchGesture::Tap);
    }

    #[test]
    fn test_touch_long_press_recognized() {
        let mut state = make_state();
        state.touch_start(TouchPoint {
            id: 1,
            x: 400.0,
            y: 250.0,
        });
        // Simulate holding for 30+ frames without moving
        for _ in 0..35 {
            state.touch_tick();
        }
        let gesture = state.touch_end(1);
        assert_eq!(gesture, TouchGesture::LongPress);
    }

    #[test]
    fn test_touch_pan_recognized() {
        let mut state = make_state();
        state.touch_start(TouchPoint {
            id: 1,
            x: 400.0,
            y: 250.0,
        });
        // Move beyond tap threshold
        state.touch_move(TouchPoint {
            id: 1,
            x: 430.0,
            y: 250.0,
        });
        assert_eq!(state.touch.gesture, TouchGesture::Pan);
        let gesture = state.touch_end(1);
        assert_eq!(gesture, TouchGesture::Pan);
    }

    #[test]
    fn test_touch_pan_scrolls_chart() {
        let mut state = make_state();
        state.consume_mask();
        let offset_before = state.time_scale.scroll_offset;
        state.touch_start(TouchPoint {
            id: 1,
            x: 400.0,
            y: 250.0,
        });
        // Move right (should scroll chart left = see newer data)
        state.touch_move(TouchPoint {
            id: 1,
            x: 430.0,
            y: 250.0,
        });
        state.touch_move(TouchPoint {
            id: 1,
            x: 460.0,
            y: 250.0,
        });
        let offset_after = state.time_scale.scroll_offset;
        assert_ne!(
            offset_before, offset_after,
            "scroll offset should have changed"
        );
    }

    #[test]
    fn test_touch_pan_produces_light_level() {
        let mut state = make_state();
        state.consume_mask();
        state.touch_start(TouchPoint {
            id: 1,
            x: 400.0,
            y: 250.0,
        });
        state.touch_move(TouchPoint {
            id: 1,
            x: 430.0,
            y: 250.0,
        });
        assert_eq!(state.invalidation_level(), InvalidationLevel::Light);
    }

    #[test]
    fn test_touch_pinch_recognized() {
        let mut state = make_state();
        state.touch_start(TouchPoint {
            id: 1,
            x: 300.0,
            y: 250.0,
        });
        state.touch_start(TouchPoint {
            id: 2,
            x: 500.0,
            y: 250.0,
        });
        assert_eq!(state.touch.gesture, TouchGesture::Pinch);
    }

    #[test]
    fn test_touch_pinch_zooms_chart() {
        let mut state = make_state();
        let spacing_before = state.time_scale.bar_spacing;
        state.touch_start(TouchPoint {
            id: 1,
            x: 300.0,
            y: 250.0,
        });
        state.touch_start(TouchPoint {
            id: 2,
            x: 500.0,
            y: 250.0,
        });
        // Move fingers apart (zoom in)
        state.touch_move(TouchPoint {
            id: 1,
            x: 250.0,
            y: 250.0,
        });
        state.touch_move(TouchPoint {
            id: 2,
            x: 550.0,
            y: 250.0,
        });
        let spacing_after = state.time_scale.bar_spacing;
        assert!(
            spacing_after > spacing_before,
            "pinch apart should zoom in (wider bars)"
        );
    }

    #[test]
    fn test_touch_pinch_to_pan_transition() {
        let mut state = make_state();
        state.touch_start(TouchPoint {
            id: 1,
            x: 300.0,
            y: 250.0,
        });
        state.touch_start(TouchPoint {
            id: 2,
            x: 500.0,
            y: 250.0,
        });
        assert_eq!(state.touch.gesture, TouchGesture::Pinch);
        // Lift one finger → should transition to Pan
        state.touch_end(2);
        // touches.len() == 1, gesture should be Pan
        assert_eq!(state.touch.gesture, TouchGesture::Pan);
    }

    #[test]
    fn test_touch_tick_advances_counter() {
        let mut state = make_state();
        assert_eq!(state.touch.frames_since_down, 0);
        state.touch_start(TouchPoint {
            id: 1,
            x: 400.0,
            y: 250.0,
        });
        state.touch_tick();
        state.touch_tick();
        assert_eq!(state.touch.frames_since_down, 2);
    }

    #[test]
    fn test_touch_tick_no_op_when_no_touches() {
        let mut state = make_state();
        state.touch_tick();
        state.touch_tick();
        assert_eq!(
            state.touch.frames_since_down, 0,
            "should not advance when no touches"
        );
    }

    // ── Per-pane interaction tests ──────────────────────────

    #[test]
    fn test_active_pane_tracks_hover() {
        let mut state = make_state();
        state.add_pane(1.0); // now 2 panes

        // Hover over the middle of pane 0
        let pane0_mid_y = state.panes[0].layout_rect.y + state.panes[0].layout_rect.height / 2.0;
        let x = state.panes[0].layout_rect.x + 10.0;
        state.pointer_move(x, pane0_mid_y);
        assert_eq!(state.active_pane, 0, "hovering pane 0 → active_pane = 0");

        // Hover over the middle of pane 1
        let pane1_mid_y = state.panes[1].layout_rect.y + state.panes[1].layout_rect.height / 2.0;
        state.pointer_move(x, pane1_mid_y);
        assert_eq!(state.active_pane, 1, "hovering pane 1 → active_pane = 1");
    }

    #[test]
    fn test_y_axis_drag_targets_active_pane() {
        let mut state = make_state();
        state.add_pane(1.0);

        // Remember pane 1 price range before drag
        let orig_min = state.panes[1].price_scale.min_price;
        let orig_max = state.panes[1].price_scale.max_price;
        let pane0_min_before = state.panes[0].price_scale.min_price;
        let pane0_max_before = state.panes[0].price_scale.max_price;

        // Simulate Y-axis drag on pane 1
        let y_axis_x = state.layout.plot_area.x + state.layout.plot_area.width + 5.0;
        let pane1_mid_y = state.panes[1].layout_rect.y + state.panes[1].layout_rect.height / 2.0;

        state.pointer_down(y_axis_x, pane1_mid_y, 0);
        assert_eq!(state.active_pane, 1, "pointer_down on pane 1's y-axis");

        // Drag 50px down (zoom out pane 1)
        state.pointer_move(y_axis_x, pane1_mid_y + 50.0);

        // Pane 1's scale should have changed
        assert_ne!(state.panes[1].price_scale.min_price, orig_min);
        assert_ne!(state.panes[1].price_scale.max_price, orig_max);

        // Pane 0's scale should be UNCHANGED
        assert!(
            (state.panes[0].price_scale.min_price - pane0_min_before).abs() < 1e-6,
            "pane 0 min_price unchanged"
        );
        assert!(
            (state.panes[0].price_scale.max_price - pane0_max_before).abs() < 1e-6,
            "pane 0 max_price unchanged"
        );
    }

    #[test]
    fn test_active_pane_set_on_pointer_down() {
        let mut state = make_state();
        state.add_pane(1.0);
        assert_eq!(state.active_pane, 0);

        let pane1_mid_y = state.panes[1].layout_rect.y + state.panes[1].layout_rect.height / 2.0;
        let x = state.panes[1].layout_rect.x + 10.0;
        state.pointer_down(x, pane1_mid_y, 0);
        // pointer_down on plot area doesn't explicitly set active_pane for Y-axis
        // but pointer_move on that position does
        state.pointer_move(x, pane1_mid_y);
        assert_eq!(state.active_pane, 1);
    }

    #[test]
    fn test_crosshair_price_uses_active_pane_scale() {
        let mut state = make_state();
        state.add_pane(1.0);

        // Manually set different scale ranges for each pane
        state.panes[0].price_scale.min_price = 100.0;
        state.panes[0].price_scale.max_price = 200.0;
        state.panes[1].price_scale.min_price = 0.0;
        state.panes[1].price_scale.max_price = 50.0;

        // Hover over pane 1 — crosshair price should use pane 1's scale
        let pane1_mid_y = state.panes[1].layout_rect.y + state.panes[1].layout_rect.height / 2.0;
        let x = state.panes[1].layout_rect.x + 10.0;
        state.pointer_move(x, pane1_mid_y);

        assert_eq!(state.active_pane, 1);
        // Price should be roughly in pane 1's 0-50 range, not pane 0's 100-200
        if let Some(price) = state.crosshair.price {
            assert!(
                price >= -10.0 && price <= 60.0,
                "price {} should be in pane 1's range (0-50), not pane 0's (100-200)",
                price
            );
        } else {
            panic!("crosshair.price should be Some");
        }
    }

    // ========================================================================
    // Time Index Map + Unified Time Axis Tests
    // ========================================================================

    #[test]
    fn test_time_index_map_matches_bar_times() {
        let bars = vec![
            make_bar(100, 50.0),
            make_bar(200, 60.0),
            make_bar(300, 70.0),
        ];
        let data = ChartData { bars };
        let state = ChartState::new(data, 800.0, 500.0, 1.0);

        // Every bar time should map to a sequential index
        assert_eq!(state.time_index_map.get(&100), Some(&0));
        assert_eq!(state.time_index_map.get(&200), Some(&1));
        assert_eq!(state.time_index_map.get(&300), Some(&2));
        assert_eq!(state.time_index_map.len(), 3);
        assert_eq!(state.time_points.len(), 3);
    }

    #[test]
    fn test_unified_time_axis_includes_series_timestamps() {
        // Create bars at times 100, 200, 300
        let bars = vec![
            make_bar(100, 50.0),
            make_bar(200, 60.0),
            make_bar(300, 70.0),
        ];
        let data = ChartData { bars };
        let mut state = ChartState::new(data, 800.0, 500.0, 1.0);

        // Add a line series with a point at time 150 (between bars)
        let mut series = crate::series::Series::line(
            0,
            vec![
                crate::series::LineDataPoint {
                    time: 150,
                    value: 55.0,
                },
                crate::series::LineDataPoint {
                    time: 250,
                    value: 65.0,
                },
            ],
        );
        series.pane_index = 0;
        state.add_series(series);

        // Unified timeline should have 5 points: 100, 150, 200, 250, 300
        assert_eq!(state.time_points.len(), 5);
        assert_eq!(state.time_points, vec![100, 150, 200, 250, 300]);
        assert_eq!(state.time_index_map.get(&100), Some(&0));
        assert_eq!(state.time_index_map.get(&150), Some(&1));
        assert_eq!(state.time_index_map.get(&200), Some(&2));
        assert_eq!(state.time_index_map.get(&250), Some(&3));
        assert_eq!(state.time_index_map.get(&300), Some(&4));
        assert_eq!(state.time_scale.bar_count, 5);
    }

    #[test]
    fn test_series_data_changed_rebuilds_time_index() {
        let bars = vec![make_bar(100, 50.0), make_bar(200, 60.0)];
        let data = ChartData { bars };
        let mut state = ChartState::new(data, 800.0, 500.0, 1.0);
        assert_eq!(state.time_points.len(), 2);

        // Add a series — should rebuild time index
        let series = crate::series::Series::line(
            0,
            vec![crate::series::LineDataPoint {
                time: 150,
                value: 55.0,
            }],
        );
        let sid = state.add_series(series);
        assert_eq!(state.time_points.len(), 3);

        // Mutate series data directly and call series_data_changed
        if let Some(s) = state.series.series.iter_mut().find(|s| s.id == sid) {
            s.data = crate::series::SeriesData::Line(vec![
                crate::series::LineDataPoint {
                    time: 150,
                    value: 55.0,
                },
                crate::series::LineDataPoint {
                    time: 175,
                    value: 57.0,
                },
            ]);
        }
        state.series_data_changed();
        assert_eq!(state.time_points.len(), 4); // 100, 150, 175, 200
    }

    #[test]
    fn test_remove_series_updates_time_index() {
        let bars = vec![make_bar(100, 50.0), make_bar(200, 60.0)];
        let data = ChartData { bars };
        let mut state = ChartState::new(data, 800.0, 500.0, 1.0);

        // Add series with extra time point
        let series = crate::series::Series::line(
            0,
            vec![crate::series::LineDataPoint {
                time: 150,
                value: 55.0,
            }],
        );
        let sid = state.add_series(series);
        assert_eq!(state.time_points.len(), 3); // 100, 150, 200

        // Remove series — time 150 should disappear
        state.remove_series(sid);
        assert_eq!(state.time_points.len(), 2); // 100, 200
        assert!(state.time_index_map.get(&150).is_none());
    }

    // ========================================================================
    // Multi-Pane Price Scale Isolation Tests
    // ========================================================================

    #[test]
    fn test_pane_price_scale_isolation_y_drag() {
        // Setup: 2 panes, each with different price ranges
        let bars = vec![make_bar(100, 150.0), make_bar(200, 160.0)];
        let data = ChartData { bars };
        let mut state = ChartState::new(data, 800.0, 600.0, 1.0);
        state.add_pane(1.0);

        // Add a series to pane 1 with very different values
        let mut series = crate::series::Series::line(
            0,
            vec![
                crate::series::LineDataPoint {
                    time: 100,
                    value: 10.0,
                },
                crate::series::LineDataPoint {
                    time: 200,
                    value: 20.0,
                },
            ],
        );
        series.pane_index = 1;
        state.add_series(series);

        // Record pane 0 and pane 1 price ranges
        let p0_min = state.panes[0].price_scale.min_price;
        let p0_max = state.panes[0].price_scale.max_price;
        let p1_min = state.panes[1].price_scale.min_price;
        let p1_max = state.panes[1].price_scale.max_price;

        // Verify they are in different ranges
        assert!(p0_min > 100.0, "pane 0 min should be >100, got {}", p0_min);
        assert!(p1_max < 100.0, "pane 1 max should be <100, got {}", p1_max);

        // Drag on pane 1's Y-axis (price scale drag)
        let p1_y = state.panes[1].layout_rect.y + state.panes[1].layout_rect.height / 2.0;
        let gutter_x = state.layout.plot_area.x + state.layout.plot_area.width + 5.0;

        // Simulate Y-axis drag start on pane 1
        state.pointer_down(gutter_x, p1_y, 0);
        state.pointer_move(gutter_x, p1_y + 50.0); // Drag down = zoom out

        // Pane 0's price scale should NOT have changed
        assert!(
            (state.panes[0].price_scale.min_price - p0_min).abs() < 1e-6,
            "pane 0 min changed from {} to {}",
            p0_min,
            state.panes[0].price_scale.min_price
        );
        assert!(
            (state.panes[0].price_scale.max_price - p0_max).abs() < 1e-6,
            "pane 0 max changed from {} to {}",
            p0_max,
            state.panes[0].price_scale.max_price
        );
    }

    #[test]
    fn test_pane_price_auto_fit_per_pane() {
        // Setup: 2 panes
        let bars = vec![
            make_bar(100, 1000.0),
            make_bar(200, 1100.0),
            make_bar(300, 1200.0),
        ];
        let data = ChartData { bars };
        let mut state = ChartState::new(data, 800.0, 600.0, 1.0);
        state.add_pane(1.0);

        // Add histogram series to pane 1 with small values near zero
        let mut series = crate::series::Series::histogram(
            0,
            vec![
                crate::series::HistogramDataPoint {
                    time: 100,
                    value: 5.0,
                    color: None,
                },
                crate::series::HistogramDataPoint {
                    time: 200,
                    value: -3.0,
                    color: None,
                },
                crate::series::HistogramDataPoint {
                    time: 300,
                    value: 8.0,
                    color: None,
                },
            ],
        );
        series.pane_index = 1;
        state.add_series(series);

        // Pane 0 should be scaled to OHLC range (~1000)
        let p0_range = state.panes[0].price_scale.max_price - state.panes[0].price_scale.min_price;
        assert!(
            p0_range > 100.0,
            "pane 0 range should be >100 (OHLC ~1000s), got {}",
            p0_range
        );

        // Pane 1 should be scaled to histogram range (~-3 to 8)
        let p1_min = state.panes[1].price_scale.min_price;
        let p1_max = state.panes[1].price_scale.max_price;
        assert!(
            p1_min < 0.0,
            "pane 1 min should be <0 for histogram, got {}",
            p1_min
        );
        assert!(p1_max < 100.0, "pane 1 max should be <100, got {}", p1_max);
        assert!(p1_max > 5.0, "pane 1 max should be >5, got {}", p1_max);
    }

    #[test]
    fn test_x_axis_zoom_affects_all_panes() {
        // Setup: 2 panes sharing the time axis
        let bars = vec![
            make_bar(100, 150.0),
            make_bar(200, 160.0),
            make_bar(300, 170.0),
        ];
        let data = ChartData { bars };
        let mut state = ChartState::new(data, 800.0, 600.0, 1.0);
        state.add_pane(1.0);

        let original_spacing = state.time_scale.bar_spacing;

        // Zoom out via the time scale (X-axis zoom affects all panes equally)
        let center_x = state.layout.plot_area.x + state.layout.plot_area.width / 2.0;
        state
            .time_scale
            .zoom(0.5, center_x, &state.layout.plot_area);

        let new_spacing = state.time_scale.bar_spacing;
        assert!(
            new_spacing < original_spacing,
            "zoom out should decrease bar spacing: {} -> {}",
            original_spacing,
            new_spacing
        );

        // Both panes see the same time scale (they share it)
        // Verify by checking that index_to_x returns same values for both panes
        let x_pane0 = state.time_scale.index_to_x(1, &state.panes[0].layout_rect);
        let x_pane1 = state.time_scale.index_to_x(1, &state.panes[1].layout_rect);
        // X should be the same because both panes have same x, width in their layout_rect
        assert!(
            (x_pane0 - x_pane1).abs() < 0.01,
            "x mismatch between panes: {} vs {}",
            x_pane0,
            x_pane1
        );
    }

    // ========================================================================
    // Whitespace / Gap Regression Tests
    // ========================================================================

    #[test]
    fn test_whitespace_gap_in_series_creates_non_adjacent_indices() {
        // Bars at times 100, 200, 300, 400, 500
        let bars = vec![
            make_bar(100, 50.0),
            make_bar(200, 60.0),
            make_bar(300, 70.0),
            make_bar(400, 80.0),
            make_bar(500, 90.0),
        ];
        let data = ChartData { bars };
        let mut state = ChartState::new(data, 800.0, 500.0, 1.0);

        // Add a line series that skips time 300 (creating a gap)
        let series = crate::series::Series::line(
            0,
            vec![
                crate::series::LineDataPoint {
                    time: 100,
                    value: 10.0,
                },
                crate::series::LineDataPoint {
                    time: 200,
                    value: 20.0,
                },
                // Gap at time 300 — series has no data here
                crate::series::LineDataPoint {
                    time: 400,
                    value: 40.0,
                },
                crate::series::LineDataPoint {
                    time: 500,
                    value: 50.0,
                },
            ],
        );
        state.add_series(series);

        // Series data point at time 200 maps to index 1
        // Series data point at time 400 maps to index 3
        // The bar index gap (1 → 3) is non-adjacent, which the renderer
        // uses to break the line path (gap detection)
        let idx_200 = *state.time_index_map.get(&200).unwrap();
        let idx_400 = *state.time_index_map.get(&400).unwrap();
        assert!(
            idx_400 > idx_200 + 1,
            "indices should be non-adjacent for gap detection: {} and {}",
            idx_200,
            idx_400
        );
    }

    #[test]
    fn test_whitespace_only_series_expands_timeline() {
        let bars = vec![make_bar(100, 50.0), make_bar(300, 70.0)];
        let data = ChartData { bars };
        let mut state = ChartState::new(data, 800.0, 500.0, 1.0);
        assert_eq!(state.time_points.len(), 2); // 100, 300

        // Add a series with time 200 — fills in the "whitespace" between bars
        let series = crate::series::Series::line(
            0,
            vec![crate::series::LineDataPoint {
                time: 200,
                value: 60.0,
            }],
        );
        state.add_series(series);
        assert_eq!(state.time_points.len(), 3); // 100, 200, 300

        // Index positions are contiguous
        assert_eq!(*state.time_index_map.get(&100).unwrap(), 0);
        assert_eq!(*state.time_index_map.get(&200).unwrap(), 1);
        assert_eq!(*state.time_index_map.get(&300).unwrap(), 2);
    }

    #[test]
    fn test_duplicate_timestamps_across_bars_and_series() {
        // Bars at 100, 200
        let bars = vec![make_bar(100, 50.0), make_bar(200, 60.0)];
        let data = ChartData { bars };
        let mut state = ChartState::new(data, 800.0, 500.0, 1.0);

        // Add series with overlapping timestamps — should not create duplicates
        let series = crate::series::Series::line(
            0,
            vec![
                crate::series::LineDataPoint {
                    time: 100,
                    value: 10.0,
                },
                crate::series::LineDataPoint {
                    time: 200,
                    value: 20.0,
                },
            ],
        );
        state.add_series(series);

        // Timeline should still be just 2 points (deduplication)
        assert_eq!(state.time_points.len(), 2);
        assert_eq!(state.time_index_map.len(), 2);
    }

    #[test]
    fn test_multi_resolution_panes_share_unified_time_axis() {
        // Scenario: 5-min main chart (10:20, 10:25) + 1-min series in pane 1
        // Timestamps: 620=10:20, 621=10:21, ..., 625=10:25 (minutes as i64)
        let bars = vec![make_bar(620, 100.0), make_bar(625, 105.0)];
        let data = ChartData { bars };
        let mut state = ChartState::new(data, 800.0, 600.0, 1.0);
        state.add_pane(1.0);

        // Add 1-min resolution series to pane 1
        let mut series = crate::series::Series::line(
            0,
            vec![
                crate::series::LineDataPoint {
                    time: 620,
                    value: 10.0,
                },
                crate::series::LineDataPoint {
                    time: 621,
                    value: 11.0,
                },
                crate::series::LineDataPoint {
                    time: 622,
                    value: 12.0,
                },
                crate::series::LineDataPoint {
                    time: 623,
                    value: 13.0,
                },
                crate::series::LineDataPoint {
                    time: 624,
                    value: 14.0,
                },
                crate::series::LineDataPoint {
                    time: 625,
                    value: 15.0,
                },
            ],
        );
        series.pane_index = 1;
        state.add_series(series);

        // CHEAP O(1) CHECK: unified timeline length = union of all unique timestamps
        // 5-min bars contribute {620, 625}, 1-min series contributes {620..625}
        // Union = {620, 621, 622, 623, 624, 625} = 6 points
        assert_eq!(state.time_points.len(), 6);
        assert_eq!(state.time_scale.bar_count, 6);

        // Both panes produce identical x-coordinates for shared timestamps
        // because there is architecturally ONE time_scale (O(1) spot check)
        let x_620_p0 = state.time_scale.index_to_x(
            *state.time_index_map.get(&620).unwrap(),
            &state.panes[0].layout_rect,
        );
        let x_620_p1 = state.time_scale.index_to_x(
            *state.time_index_map.get(&620).unwrap(),
            &state.panes[1].layout_rect,
        );
        assert!(
            (x_620_p0 - x_620_p1).abs() < 0.01,
            "panes must share x-axis: pane0={}, pane1={}",
            x_620_p0,
            x_620_p1
        );

        // Pane 0's 5-min bar at time 625 sits at unified index 5, not index 1
        assert_eq!(*state.time_index_map.get(&625).unwrap(), 5);
    }
}
