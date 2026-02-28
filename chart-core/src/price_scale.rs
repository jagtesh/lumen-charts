use crate::chart_model::{OhlcBar, Rect};

/// Maps price values to Y pixel coordinates (inverted: high price = low y)
pub struct PriceScale {
    pub min_price: f64,
    pub max_price: f64,
}

impl PriceScale {
    /// Auto-fit to data with 5% margin
    pub fn from_data(bars: &[OhlcBar]) -> Self {
        if bars.is_empty() {
            return PriceScale {
                min_price: 0.0,
                max_price: 100.0,
            };
        }
        let min = bars.iter().map(|b| b.low).fold(f64::INFINITY, f64::min);
        let max = bars
            .iter()
            .map(|b| b.high)
            .fold(f64::NEG_INFINITY, f64::max);
        let range = max - min;
        let margin = range * 0.05;
        PriceScale {
            min_price: min - margin,
            max_price: max + margin,
        }
    }

    /// Convert price value to y pixel coordinate
    pub fn price_to_y(&self, price: f64, plot_area: &Rect) -> f32 {
        let range = self.max_price - self.min_price;
        if range == 0.0 {
            return plot_area.y + plot_area.height * 0.5;
        }
        let normalized = (price - self.min_price) / range;
        plot_area.y + plot_area.height * (1.0 - normalized as f32)
    }
}
