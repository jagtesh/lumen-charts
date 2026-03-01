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
    ///
    /// Relationship: `first = bar_count - visible + scroll_offset`
    /// Inverse:      `scroll_offset = first - bar_count + visible`
    /// Use `scroll_offset_for_first()` when computing the inverse.
    pub fn first_visible_index(&self, plot_width: f32) -> f32 {
        let visible = self.visible_bar_count(plot_width);
        self.bar_count as f32 - visible + self.scroll_offset
    }

    /// Index of the last visible bar
    pub fn last_visible_index(&self, _plot_width: f32) -> f32 {
        self.bar_count as f32 + self.scroll_offset
    }

    /// Compute the scroll_offset that would place `first` as the first visible index.
    /// This is the inverse of `first_visible_index()`.
    pub fn scroll_offset_for_first(&self, first: f32, plot_width: f32) -> f32 {
        let visible = plot_width / self.bar_spacing;
        first - self.bar_count as f32 + visible
    }

    /// Convert bar index to center x pixel coordinate within the plot area
    pub fn index_to_x(&self, index: usize, plot_area: &Rect) -> f32 {
        self.logical_to_x(index as f32, plot_area)
    }

    /// Convert fractional logical index to center x pixel coordinate
    pub fn logical_to_x(&self, logical: f32, plot_area: &Rect) -> f32 {
        let first = self.first_visible_index(plot_area.width);
        plot_area.x + (logical - first + 0.5) * self.bar_spacing
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

    /// Scroll by a number of bars (negative = see older data, positive = see newer)
    pub fn scroll_by(&mut self, delta_bars: f32) {
        self.scroll_offset += delta_bars;
        self.clamp_scroll();
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
        self.scroll_offset = self.scroll_offset_for_first(new_first, plot_area.width);
        self.clamp_scroll();
    }

    pub fn clamp_scroll(&mut self) {
        // Negative = scrolled into history, positive = past the end
        let max_history = (self.bar_count as f32).max(0.0);
        self.scroll_offset = self.scroll_offset.clamp(-max_history, 10.0);
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
        ts.scroll_by(-2000.0);
        assert!(ts.scroll_offset >= -100.0);

        ts.scroll_by(3000.0);
        assert!(ts.scroll_offset <= 10.0);
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
    fn test_zoom_stability() {
        // Zooming in then out by the same factor should return to roughly the same state
        let mut ts = TimeScale::new(100, 800.0);
        let area = make_plot_area(800.0);
        let center = area.x + area.width / 2.0;
        let orig_spacing = ts.bar_spacing;
        let orig_offset = ts.scroll_offset;

        ts.zoom(1.5, center, &area);
        ts.zoom(1.0 / 1.5, center, &area);

        assert!(
            (ts.bar_spacing - orig_spacing).abs() < 0.1,
            "spacing: {} vs {}",
            ts.bar_spacing,
            orig_spacing
        );
        assert!(
            (ts.scroll_offset - orig_offset).abs() < 1.0,
            "offset: {} vs {}",
            ts.scroll_offset,
            orig_offset
        );
    }

    #[test]
    fn test_zoom_center_pinned() {
        // The bar under the center point should stay at the same x position after zoom
        let mut ts = TimeScale::new(100, 800.0);
        let area = make_plot_area(800.0);
        let center = area.x + 300.0;

        let idx_before = ts.x_to_index(center, &area);
        ts.zoom(2.0, center, &area);
        let x_after = ts.index_to_x(idx_before.round() as usize, &area);

        assert!(
            (x_after - center).abs() < ts.bar_spacing,
            "center shifted: {} vs {}",
            x_after,
            center
        );
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
