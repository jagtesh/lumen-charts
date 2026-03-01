//! Shared `Scale` trait for TimeScale and PriceScale.
//!
//! Both scales convert between domain values (time/price) and pixel coordinates.
//! This trait captures the shared behavior so that sub-API objects (ITimeScaleApi,
//! IPriceScaleApi) can be built from a common interface.

use crate::chart_model::Rect;

/// Shared behavior for any scale axis (time or price).
///
/// A `Scale` maps between "logical" domain values (e.g., bar index or price)
/// and pixel coordinates within a given plot area.
pub trait Scale {
    /// Convert a domain value to a pixel coordinate.
    fn value_to_coordinate(&self, value: f64, rect: &Rect) -> f32;

    /// Convert a pixel coordinate to a domain value.
    fn coordinate_to_value(&self, coord: f32, rect: &Rect) -> f64;

    /// Get the current visible range as (min, max) in domain units.
    fn visible_range(&self, rect: &Rect) -> (f64, f64);
}

// -- Implementations --

impl Scale for crate::time_scale::TimeScale {
    fn value_to_coordinate(&self, value: f64, rect: &Rect) -> f32 {
        self.logical_to_x(value as f32, rect)
    }

    fn coordinate_to_value(&self, coord: f32, rect: &Rect) -> f64 {
        self.x_to_index(coord, rect) as f64
    }

    fn visible_range(&self, rect: &Rect) -> (f64, f64) {
        let first = self.first_visible_index(rect.width) as f64;
        let last = self.last_visible_index(rect.width) as f64;
        (first, last)
    }
}

impl Scale for crate::price_scale::PriceScale {
    fn value_to_coordinate(&self, value: f64, rect: &Rect) -> f32 {
        self.price_to_y(value, rect)
    }

    fn coordinate_to_value(&self, coord: f32, rect: &Rect) -> f64 {
        self.y_to_price(coord, rect)
    }

    fn visible_range(&self, _rect: &Rect) -> (f64, f64) {
        (self.min_price, self.max_price)
    }
}

// =============================================================================
// Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::chart_model::OhlcBar;

    fn make_rect() -> Rect {
        Rect {
            x: 10.0,
            y: 20.0,
            width: 800.0,
            height: 400.0,
        }
    }

    #[test]
    fn test_time_scale_roundtrip_via_trait() {
        let ts = crate::time_scale::TimeScale::new(100, 800.0);
        let rect = make_rect();
        let value = 50.0_f64;
        let coord = ts.value_to_coordinate(value, &rect);
        let back = ts.coordinate_to_value(coord, &rect);
        assert!(
            (back - value).abs() < 1.0,
            "roundtrip within 1 bar: got {back} vs {value}"
        );
    }

    #[test]
    fn test_price_scale_roundtrip_via_trait() {
        let bars = vec![OhlcBar {
            time: 0,
            open: 100.0,
            high: 200.0,
            low: 100.0,
            close: 150.0,
        }];
        let ps = crate::price_scale::PriceScale::from_data(&bars);
        let rect = make_rect();
        let value = 150.0_f64;
        let coord = ps.value_to_coordinate(value, &rect);
        let back = ps.coordinate_to_value(coord, &rect);
        assert!(
            (back - value).abs() < 0.1,
            "roundtrip within 0.1: got {back} vs {value}"
        );
    }

    #[test]
    fn test_time_scale_visible_range() {
        let ts = crate::time_scale::TimeScale::new(100, 800.0);
        let rect = make_rect();
        let (first, last) = Scale::visible_range(&ts, &rect);
        assert!(first >= 0.0);
        assert!(last > first);
        assert!(last <= 100.0);
    }

    #[test]
    fn test_price_scale_visible_range() {
        let bars = vec![OhlcBar {
            time: 0,
            open: 100.0,
            high: 200.0,
            low: 100.0,
            close: 150.0,
        }];
        let ps = crate::price_scale::PriceScale::from_data(&bars);
        let rect = make_rect();
        let (min, max) = ps.visible_range(&rect);
        assert!(min < 100.0, "min should include margin below data");
        assert!(max > 200.0, "max should include margin above data");
    }

    #[test]
    fn test_both_scales_implement_trait() {
        // Compile-time test: both types can be used as `dyn Scale`
        fn accepts_scale(_s: &dyn Scale) {}
        let ts = crate::time_scale::TimeScale::new(50, 400.0);
        let ps = crate::price_scale::PriceScale::from_data(&[]);
        accepts_scale(&ts);
        accepts_scale(&ps);
    }
}
