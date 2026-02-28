use crate::chart_model::OhlcBar;
use crate::chart_model::Rect;
use crate::price_scale::PriceScale;
use crate::time_scale::TimeScale;

/// A tick mark with its position and label text
pub struct TickMark {
    pub value: f64,
    pub label: String,
    pub coord: f32,
}

/// Find a "nice" step size for tick marks (1, 2, 5, 10, 20, 50, ...)
fn nice_step(range: f64, target_count: usize) -> f64 {
    let rough_step = range / target_count as f64;
    let magnitude = 10.0_f64.powf(rough_step.log10().floor());
    let residual = rough_step / magnitude;
    let nice = if residual <= 1.5 {
        1.0
    } else if residual <= 3.5 {
        2.0
    } else if residual <= 7.5 {
        5.0
    } else {
        10.0
    };
    nice * magnitude
}

/// Generate ~6-8 nicely spaced price tick marks
pub fn generate_price_ticks(scale: &PriceScale, plot_area: &Rect) -> Vec<TickMark> {
    let range = scale.max_price - scale.min_price;
    if range <= 0.0 {
        return vec![];
    }
    let step = nice_step(range, 7);
    let start = (scale.min_price / step).ceil() * step;

    let mut ticks = Vec::new();
    let mut value = start;
    while value <= scale.max_price {
        let coord = scale.price_to_y(value, plot_area);
        // Format with appropriate precision
        let label = if step >= 1.0 {
            format!("{:.0}", value)
        } else if step >= 0.1 {
            format!("{:.1}", value)
        } else {
            format!("{:.2}", value)
        };
        ticks.push(TickMark {
            value,
            label,
            coord,
        });
        value += step;
    }
    ticks
}

/// Generate time tick marks (every Nth bar)
pub fn generate_time_ticks(
    bars: &[OhlcBar],
    time_scale: &TimeScale,
    plot_area: &Rect,
) -> Vec<TickMark> {
    if bars.is_empty() {
        return vec![];
    }

    // Aim for ~6-10 labels
    let target_labels = 8;
    let step = (bars.len() / target_labels).max(1);

    let mut ticks = Vec::new();
    let mut i = 0;
    while i < bars.len() {
        let x = time_scale.index_to_x(i, plot_area);
        let ts = bars[i].time;

        // Format as date
        let label = if let Some(dt) = chrono::DateTime::from_timestamp(ts, 0) {
            dt.format("%b %d").to_string()
        } else {
            format!("{}", i)
        };

        ticks.push(TickMark {
            value: ts as f64,
            label,
            coord: x,
        });
        i += step;
    }
    ticks
}
