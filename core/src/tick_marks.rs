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
    if rough_step <= 0.0 {
        return 1.0;
    }
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

/// Find a "nice" integer step for bar indices (1, 2, 5, 10, 20, 50, ...)
fn nice_index_step(visible_count: usize, target_labels: usize) -> usize {
    let rough = visible_count as f64 / target_labels as f64;
    if rough <= 1.0 {
        return 1;
    }
    let magnitude = 10.0_f64.powf(rough.log10().floor());
    let residual = rough / magnitude;
    let nice = if residual <= 1.5 {
        1.0
    } else if residual <= 3.5 {
        2.0
    } else if residual <= 7.5 {
        5.0
    } else {
        10.0
    };
    (nice * magnitude).round().max(1.0) as usize
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

/// Generate time tick marks anchored to absolute bar indices.
///
/// Ticks are always placed at multiples of a "nice" step relative to index 0,
/// so they don't jump when the user pans. Only ticks within the visible
/// plot area are emitted.
pub fn generate_time_ticks(
    bars: &[OhlcBar],
    time_scale: &TimeScale,
    plot_area: &Rect,
) -> Vec<TickMark> {
    if bars.is_empty() {
        return vec![];
    }

    let (first_vis, last_vis) = time_scale.visible_range(plot_area.width);
    let visible_count = last_vis.saturating_sub(first_vis);
    if visible_count == 0 {
        return vec![];
    }

    // Compute a "nice" step that produces ~6-8 labels
    let step = nice_index_step(visible_count, 8);

    // Align to absolute multiples of step (anchored at index 0, not first_vis)
    // This prevents ticks from jumping as the user pans.
    let start = if first_vis == 0 {
        0
    } else {
        ((first_vis / step) + 1) * step
    };

    let mut ticks = Vec::new();
    let mut i = start;
    while i < last_vis && i < bars.len() {
        let x = time_scale.index_to_x(i, plot_area);

        // Only include ticks within the plot area (with small margin)
        if x >= plot_area.x - 10.0 && x <= plot_area.x + plot_area.width + 10.0 {
            let ts = bars[i].time;
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
        }
        i += step;
    }
    ticks
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_nice_index_step() {
        assert_eq!(nice_index_step(100, 8), 10);
        assert_eq!(nice_index_step(50, 8), 5);
        assert_eq!(nice_index_step(200, 8), 20);
        assert_eq!(nice_index_step(5, 8), 1);
    }

    #[test]
    fn test_time_ticks_stable_during_pan() {
        // Simulates panning: changing first_vis should NOT change which
        // absolute bar indices get ticks, only which are visible.
        let bars: Vec<OhlcBar> = (0..200)
            .map(|i| OhlcBar {
                time: 1700000000 + i * 86400,
                open: 100.0,
                high: 110.0,
                low: 90.0,
                close: 105.0,
            })
            .collect();

        let area = Rect {
            x: 10.0,
            y: 20.0,
            width: 800.0,
            height: 400.0,
        };

        // Create two time scales at different scroll positions
        let mut ts1 = TimeScale::new(200, 800.0);
        let mut ts2 = TimeScale::new(200, 800.0);
        ts2.scroll_by(-5.0); // Pan slightly

        let ticks1 = generate_time_ticks(&bars, &ts1, &area);
        let ticks2 = generate_time_ticks(&bars, &ts2, &area);

        // Both should use the same step, so tick bar indices should be
        // the same set (just shifted by visibility)
        let indices1: Vec<i64> = ticks1.iter().map(|t| t.value as i64).collect();
        let indices2: Vec<i64> = ticks2.iter().map(|t| t.value as i64).collect();

        // There should be significant overlap between the two sets
        let overlap = indices1.iter().filter(|v| indices2.contains(v)).count();
        assert!(overlap > 0, "No overlapping ticks - ticks are jumping!");
    }
}
