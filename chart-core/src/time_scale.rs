use crate::chart_model::Rect;

/// Mutable time scale state — tracks scroll position and zoom level
#[derive(Debug, Clone)]
pub struct TimeScale {
    /// Pixels per bar (controls zoom level)
    pub bar_spacing: f32,
    /// Scroll offset in bars (0 = rightmost bar at right edge)
    pub scroll_offset: f32,
    /// Total number of data bars
    pub bar_count: usize,
    /// Minimum bar spacing (max zoom out)
    pub min_bar_spacing: f32,
    /// Maximum bar spacing (max zoom in)
    pub max_bar_spacing: f32,
}

impl TimeScale {
    pub fn new(bar_count: usize, plot_width: f32) -> Self {
        let bar_spacing = if bar_count > 0 {
            (plot_width / bar_count as f32).clamp(2.0, 30.0)
        } else {
            6.0
        };
        TimeScale {
            bar_spacing,
            scroll_offset: 0.0,
            bar_count,
            min_bar_spacing: 2.0,
            max_bar_spacing: 30.0,
        }
    }

    /// How many bars fit in the visible area
    pub fn visible_bar_count(&self, plot_width: f32) -> f32 {
        plot_width / self.bar_spacing
    }

    /// Index of the first visible bar (may be fractional/negative)
    pub fn first_visible_index(&self, plot_width: f32) -> f32 {
        let visible = self.visible_bar_count(plot_width);
        self.bar_count as f32 - visible + self.scroll_offset
    }

    /// Index of the last visible bar
    pub fn last_visible_index(&self, _plot_width: f32) -> f32 {
        self.bar_count as f32 + self.scroll_offset
    }

    /// Convert bar index to center x pixel coordinate within the plot area
    pub fn index_to_x(&self, index: usize, plot_area: &Rect) -> f32 {
        let first = self.first_visible_index(plot_area.width);
        plot_area.x + (index as f32 - first + 0.5) * self.bar_spacing
    }

    /// Convert x pixel coordinate to bar index (may be fractional)
    pub fn x_to_index(&self, x: f32, plot_area: &Rect) -> f32 {
        let first = self.first_visible_index(plot_area.width);
        first + (x - plot_area.x) / self.bar_spacing - 0.5
    }

    /// Snap an x coordinate to the nearest bar index (clamped)
    pub fn x_to_nearest_index(&self, x: f32, plot_area: &Rect) -> Option<usize> {
        let idx = self.x_to_index(x, plot_area).round() as i64;
        if idx >= 0 && idx < self.bar_count as i64 {
            Some(idx as usize)
        } else {
            None
        }
    }

    /// Scroll by a number of bars (positive = scroll right / see older data)
    pub fn scroll_by(&mut self, delta_bars: f32) {
        self.scroll_offset += delta_bars;
        // Clamp: can't scroll past the first bar, and only a bit past the last
        let max_scroll = (self.bar_count as f32).max(0.0);
        self.scroll_offset = self.scroll_offset.clamp(-10.0, max_scroll);
    }

    /// Scroll by pixels (converts to bar units)
    pub fn scroll_by_pixels(&mut self, delta_px: f32) {
        let delta_bars = delta_px / self.bar_spacing;
        self.scroll_by(delta_bars);
    }

    /// Zoom by a factor around a center x coordinate
    pub fn zoom(&mut self, factor: f32, center_x: f32, plot_area: &Rect) {
        let old_spacing = self.bar_spacing;
        let new_spacing = (old_spacing * factor).clamp(self.min_bar_spacing, self.max_bar_spacing);

        if (new_spacing - old_spacing).abs() < 0.001 {
            return;
        }

        // Keep the bar under center_x in the same screen position
        let center_index = self.x_to_index(center_x, plot_area);
        self.bar_spacing = new_spacing;

        // Recalculate scroll offset to keep center_index at center_x
        let new_first = center_index - (center_x - plot_area.x) / new_spacing + 0.5;
        let new_visible = plot_area.width / new_spacing;
        self.scroll_offset = self.bar_count as f32 - new_visible - new_first;
        let max_scroll = (self.bar_count as f32).max(0.0);
        self.scroll_offset = self.scroll_offset.clamp(-10.0, max_scroll);
    }

    /// Fit all bars into the visible area
    pub fn fit_content(&mut self, plot_width: f32) {
        if self.bar_count > 0 {
            self.bar_spacing = (plot_width / self.bar_count as f32)
                .clamp(self.min_bar_spacing, self.max_bar_spacing);
        }
        self.scroll_offset = 0.0;
    }

    /// Get the range of visible bar indices (clamped to data bounds)
    pub fn visible_range(&self, plot_width: f32) -> (usize, usize) {
        let first = self.first_visible_index(plot_width).floor().max(0.0) as usize;
        let last = self
            .last_visible_index(plot_width)
            .ceil()
            .min(self.bar_count as f32) as usize;
        (first, last)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_plot_area(width: f32) -> Rect {
        Rect {
            x: 10.0,
            y: 20.0,
            width,
            height: 400.0,
        }
    }

    #[test]
    fn test_new_fits_all_bars() {
        let ts = TimeScale::new(100, 800.0);
        assert_eq!(ts.bar_count, 100);
        assert!((ts.bar_spacing - 8.0).abs() < 0.01);
        assert_eq!(ts.scroll_offset, 0.0);
    }

    #[test]
    fn test_index_to_x_and_back() {
        let ts = TimeScale::new(100, 800.0);
        let area = make_plot_area(800.0);

        let x = ts.index_to_x(50, &area);
        let idx = ts.x_to_index(x, &area);
        assert!((idx - 50.0).abs() < 0.01);
    }

    #[test]
    fn test_scroll_by_bars() {
        let mut ts = TimeScale::new(100, 800.0);
        let area = make_plot_area(800.0);

        let x_before = ts.index_to_x(50, &area);
        ts.scroll_by(10.0);
        let x_after = ts.index_to_x(50, &area);

        // Scrolling right should move bars left
        assert!(x_after < x_before);
    }

    #[test]
    fn test_scroll_clamped() {
        let mut ts = TimeScale::new(100, 800.0);
        ts.scroll_by(1000.0);
        assert!(ts.scroll_offset <= 100.0);

        ts.scroll_by(-2000.0);
        assert!(ts.scroll_offset >= -10.0);
    }

    #[test]
    fn test_zoom_in() {
        let mut ts = TimeScale::new(100, 800.0);
        let area = make_plot_area(800.0);
        let old_spacing = ts.bar_spacing;

        ts.zoom(1.5, area.x + area.width / 2.0, &area);
        assert!(ts.bar_spacing > old_spacing);
    }

    #[test]
    fn test_zoom_out() {
        let mut ts = TimeScale::new(100, 800.0);
        let area = make_plot_area(800.0);
        let old_spacing = ts.bar_spacing;

        ts.zoom(0.7, area.x + area.width / 2.0, &area);
        assert!(ts.bar_spacing < old_spacing);
    }

    #[test]
    fn test_zoom_clamped() {
        let mut ts = TimeScale::new(100, 800.0);
        let area = make_plot_area(800.0);

        // Zoom way in
        ts.zoom(100.0, area.x + area.width / 2.0, &area);
        assert!(ts.bar_spacing <= ts.max_bar_spacing);

        // Zoom way out
        ts.zoom(0.01, area.x + area.width / 2.0, &area);
        assert!(ts.bar_spacing >= ts.min_bar_spacing);
    }

    #[test]
    fn test_fit_content() {
        let mut ts = TimeScale::new(100, 800.0);
        ts.scroll_by(50.0);
        ts.zoom(2.0, 400.0, &make_plot_area(800.0));

        ts.fit_content(800.0);
        assert_eq!(ts.scroll_offset, 0.0);
        assert!((ts.bar_spacing - 8.0).abs() < 0.01);
    }

    #[test]
    fn test_visible_range() {
        let ts = TimeScale::new(100, 800.0);
        let (first, last) = ts.visible_range(800.0);
        assert_eq!(first, 0);
        assert_eq!(last, 100);
    }

    #[test]
    fn test_x_to_nearest_index() {
        let ts = TimeScale::new(100, 800.0);
        let area = make_plot_area(800.0);

        let x = ts.index_to_x(42, &area);
        assert_eq!(ts.x_to_nearest_index(x, &area), Some(42));
    }

    #[test]
    fn test_x_to_nearest_index_out_of_bounds() {
        let ts = TimeScale::new(100, 800.0);
        let area = make_plot_area(800.0);

        assert_eq!(ts.x_to_nearest_index(-100.0, &area), None);
        assert_eq!(ts.x_to_nearest_index(10000.0, &area), None);
    }
}
