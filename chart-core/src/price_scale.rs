use crate::chart_model::{OhlcBar, Rect};

/// Maps price values to Y pixel coordinates (inverted: high price = low y)
#[derive(Debug, Clone)]
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

    /// Convert y pixel coordinate to price value (inverse of price_to_y)
    pub fn y_to_price(&self, y: f32, plot_area: &Rect) -> f64 {
        let range = self.max_price - self.min_price;
        if range == 0.0 {
            return self.min_price;
        }
        let normalized = 1.0 - ((y - plot_area.y) / plot_area.height) as f64;
        self.min_price + normalized * range
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_bar(low: f64, high: f64) -> OhlcBar {
        OhlcBar {
            time: 0,
            open: low,
            high,
            low,
            close: high,
        }
    }

    fn make_plot_area() -> Rect {
        Rect {
            x: 10.0,
            y: 20.0,
            width: 800.0,
            height: 400.0,
        }
    }

    #[test]
    fn test_price_to_y_and_back() {
        let bars = vec![make_bar(100.0, 200.0)];
        let ps = PriceScale::from_data(&bars);
        let area = make_plot_area();

        let price = 150.0;
        let y = ps.price_to_y(price, &area);
        let back = ps.y_to_price(y, &area);
        assert!((back - price).abs() < 0.1);
    }

    #[test]
    fn test_high_price_is_low_y() {
        let bars = vec![make_bar(100.0, 200.0)];
        let ps = PriceScale::from_data(&bars);
        let area = make_plot_area();

        let y_high = ps.price_to_y(200.0, &area);
        let y_low = ps.price_to_y(100.0, &area);
        assert!(y_high < y_low); // Higher price = lower pixel y
    }
}
