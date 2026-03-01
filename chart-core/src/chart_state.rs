use crate::chart_model::{ChartData, ChartLayout};
use crate::price_scale::PriceScale;
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

/// Drag state for panning
#[derive(Debug, Clone, Default)]
pub struct DragState {
    pub active: bool,
    pub start_x: f32,
    pub start_y: f32,
    pub last_x: f32,
    pub last_y: f32,
}

/// Full mutable chart state — owns the model+view state
pub struct ChartState {
    pub data: ChartData,
    pub time_scale: TimeScale,
    pub price_scale: PriceScale,
    pub layout: ChartLayout,
    pub crosshair: Crosshair,
    pub drag: DragState,
}

impl ChartState {
    pub fn new(data: ChartData, width: f32, height: f32, scale_factor: f64) -> Self {
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
        }
    }

    /// Recalculate layout after resize
    pub fn resize(&mut self, width: f32, height: f32, scale_factor: f64) {
        self.layout = ChartLayout::new(width, height, scale_factor);
        // Re-fit price scale to visible data
        self.update_price_scale();
    }

    /// Update price scale to fit visible data
    pub fn update_price_scale(&mut self) {
        let (first, last) = self.time_scale.visible_range(self.layout.plot_area.width);
        if first < last && last <= self.data.bars.len() {
            self.price_scale = PriceScale::from_data(&self.data.bars[first..last]);
        }
    }

    // --- Interaction handlers (all return true if redraw needed) ---

    /// Pointer moved to (x, y) in logical coordinates
    pub fn pointer_move(&mut self, x: f32, y: f32) -> bool {
        let in_plot = self.layout.plot_area_contains(x, y);
        let was_visible = self.crosshair.visible;

        if in_plot {
            self.crosshair.visible = true;
            self.crosshair.x = x;
            self.crosshair.y = y;
            self.crosshair.bar_index = self
                .time_scale
                .x_to_nearest_index(x, &self.layout.plot_area);
            self.crosshair.price = Some(self.price_scale.y_to_price(y, &self.layout.plot_area));

            // Handle drag panning
            if self.drag.active {
                let delta_px = self.drag.last_x - x; // drag left = scroll right
                self.time_scale.scroll_by_pixels(delta_px);
                self.update_price_scale();
                self.drag.last_x = x;
                self.drag.last_y = y;
            }

            true
        } else {
            self.crosshair.visible = false;
            self.crosshair.bar_index = None;
            self.crosshair.price = None;
            was_visible // only redraw if we're hiding the crosshair
        }
    }

    /// Pointer button pressed
    pub fn pointer_down(&mut self, x: f32, y: f32, _button: u8) -> bool {
        if self.layout.plot_area_contains(x, y) {
            self.drag = DragState {
                active: true,
                start_x: x,
                start_y: y,
                last_x: x,
                last_y: y,
            };
        }
        false // no visual change on down
    }

    /// Pointer button released
    pub fn pointer_up(&mut self, _x: f32, _y: f32, _button: u8) -> bool {
        let was_dragging = self.drag.active;
        self.drag.active = false;
        was_dragging
    }

    /// Pointer left the chart area
    pub fn pointer_leave(&mut self) -> bool {
        let was_visible = self.crosshair.visible;
        self.crosshair.visible = false;
        self.crosshair.bar_index = None;
        self.crosshair.price = None;
        self.drag.active = false;
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
}
