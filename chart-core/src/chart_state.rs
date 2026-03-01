use crate::chart_model::{ChartData, ChartLayout, OhlcBar};
use crate::chart_options::ChartOptions;
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

/// Full mutable chart state — owns the model+view state
pub struct ChartState {
    pub data: ChartData,
    pub time_scale: TimeScale,
    pub price_scale: PriceScale,
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

    /// Pending events from the last interaction call
    pub pending_events: InteractionEvents,
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

        ChartState {
            data,
            time_scale,
            price_scale,
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
            pending_events: InteractionEvents::default(),
        }
    }

    /// Recalculate layout after resize
    pub fn resize(&mut self, width: f32, height: f32, scale_factor: f64) {
        self.layout = ChartLayout::new(width, height, scale_factor);
        self.update_price_scale();
    }

    /// Update price scale to fit visible data
    pub fn update_price_scale(&mut self) {
        let (first, last) = self.time_scale.visible_range(self.layout.plot_area.width);
        if first < last && last <= self.data.bars.len() {
            self.price_scale = PriceScale::from_data(&self.data.bars[first..last]);
        }
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
            self.crosshair.visible = true;
            self.crosshair.x = x;
            self.crosshair.y = y;
            self.crosshair.bar_index = self
                .time_scale
                .x_to_nearest_index(x, &self.layout.plot_area);
            self.crosshair.price = Some(self.price_scale.y_to_price(y, &self.layout.plot_area));

            // Handle drag panning (plot area drag)
            if self.drag.active {
                if self.drag.zone == Some(HitZone::PlotArea) {
                    let delta_px = self.drag.last_x - x;
                    self.time_scale.scroll_by_pixels(delta_px);
                    self.update_price_scale();
                }
                let dx = x - self.drag.last_x;
                let dy = y - self.drag.last_y;
                self.drag.total_distance += (dx * dx + dy * dy).sqrt();
                self.drag.last_x = x;
                self.drag.last_y = y;
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
                self.y_axis_drag_start_range =
                    Some((self.price_scale.min_price, self.price_scale.max_price));
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
                    Some(self.price_scale.y_to_price(y, &self.layout.plot_area))
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

    /// Scroll (horizontal pan). delta_x > 0 scrolls right (sees older data)
    pub fn scroll(&mut self, delta_x: f32, _delta_y: f32) -> bool {
        if delta_x.abs() < 0.1 {
            return false;
        }
        self.time_scale.scroll_by_pixels(delta_x);
        self.update_price_scale();
        true
    }

    /// Zoom by a factor around a center x coordinate
    /// factor > 1.0 = zoom in, < 1.0 = zoom out
    pub fn zoom(&mut self, factor: f32, center_x: f32) -> bool {
        self.time_scale
            .zoom(factor, center_x, &self.layout.plot_area);
        self.update_price_scale();
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
        true
    }

    /// Keyboard input. Returns true if needs redraw.
    pub fn key_down(&mut self, key: ChartKey) -> bool {
        let scroll_amount = self.time_scale.bar_spacing * 3.0; // scroll 3 bars at a time
        match key {
            ChartKey::ArrowLeft => {
                self.time_scale.scroll_by_pixels(-scroll_amount);
                self.update_price_scale();
                true
            }
            ChartKey::ArrowRight => {
                self.time_scale.scroll_by_pixels(scroll_amount);
                self.update_price_scale();
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
            self.price_scale.min_price = mid - new_half;
            self.price_scale.max_price = mid + new_half;
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

    // --- Data management ---

    /// Replace all bar data. Resets time scale to fit new data.
    pub fn set_data(&mut self, bars: Vec<OhlcBar>) {
        self.data.bars = bars;
        self.data.bars.sort_by_key(|b| b.time);
        self.time_scale = TimeScale::new(self.data.bars.len(), self.layout.plot_area.width);
        self.update_price_scale();
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
            }
        }
        self.update_price_scale();
    }

    /// Remove and return the last (most recent) bar.
    pub fn pop_bar(&mut self) -> Option<OhlcBar> {
        let bar = self.data.bars.pop();
        if bar.is_some() {
            self.time_scale = TimeScale::new(self.data.bars.len(), self.layout.plot_area.width);
            self.update_price_scale();
        }
        bar
    }

    /// Get the number of bars.
    pub fn bar_count(&self) -> usize {
        self.data.bars.len()
    }

    // --- Options ---

    /// Apply new chart options. Returns true (always needs redraw).
    pub fn apply_options(&mut self, options: ChartOptions) -> bool {
        self.options = options;
        true
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
        let orig_range = state.price_scale.max_price - state.price_scale.min_price;

        state.pointer_down(ax, ay, 0);
        // Drag down → should zoom out (expand range)
        state.pointer_move(ax, ay + 50.0);
        let new_range = state.price_scale.max_price - state.price_scale.min_price;
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
        state.price_scale.min_price -= 50.0;
        state.price_scale.max_price += 50.0;
        let expanded_range = state.price_scale.max_price - state.price_scale.min_price;

        // Double click on Y-axis
        state.pointer_down(ax, ay, 0);
        state.pointer_up(ax, ay, 0);
        for _ in 0..3 {
            state.tick();
        }
        state.pointer_down(ax, ay, 0);
        state.pointer_up(ax, ay, 0);

        // Should have reset (auto-fit to visible data)
        let reset_range = state.price_scale.max_price - state.price_scale.min_price;
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
}
