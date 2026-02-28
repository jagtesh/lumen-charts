use crate::chart_model::Rect;

/// Maps bar indices to X pixel coordinates
pub struct TimeScale {
    pub bar_spacing: f32,
    pub bar_count: usize,
}

impl TimeScale {
    pub fn new(bar_count: usize, plot_area: &Rect) -> Self {
        let bar_spacing = if bar_count > 0 {
            plot_area.width / bar_count as f32
        } else {
            1.0
        };
        TimeScale {
            bar_spacing,
            bar_count,
        }
    }

    /// Convert bar index to center x pixel coordinate within the plot area
    pub fn index_to_x(&self, index: usize, plot_area: &Rect) -> f32 {
        plot_area.x + (index as f32 + 0.5) * self.bar_spacing
    }
}
