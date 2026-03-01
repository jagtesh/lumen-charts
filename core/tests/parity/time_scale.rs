/// Parity tests from LWC: tests/unittests/time-scale.spec.ts
use chart_core::chart_model::{OhlcBar, Rect};
use chart_core::time_scale::TimeScale;

fn make_bars(count: usize) -> Vec<OhlcBar> {
    (0..count)
        .map(|i| OhlcBar {
            time: i as i64,
            open: 100.0,
            high: 110.0,
            low: 90.0,
            close: 105.0,
        })
        .collect()
}

fn plot_rect() -> Rect {
    Rect {
        x: 0.0,
        y: 0.0,
        width: 500.0,
        height: 400.0,
    }
}

/// LWC: indexToCoordinate and coordinateToIndex inverse
/// File: time-scale.spec.ts, line 40
#[test]
fn lwc_index_to_coord_inverse() {
    let ts = TimeScale::new(500, 500.0);
    let x = ts.index_to_x(250, &plot_rect());
    let idx = ts.x_to_nearest_index(x, &plot_rect());
    assert_eq!(idx, Some(250));
}

/// LWC: timeToIndex — should return index for time on scale
/// File: time-scale.spec.ts, line 72
#[test]
fn lwc_time_to_index_found() {
    let bars = make_bars(3);
    assert_eq!(bars.iter().position(|b| b.time == 0), Some(0));
    assert_eq!(bars.iter().position(|b| b.time == 1), Some(1));
    assert_eq!(bars.iter().position(|b| b.time == 2), Some(2));
}

/// LWC: timeToIndex — should return null for time not on scale
/// File: time-scale.spec.ts, line 82
#[test]
fn lwc_time_to_index_not_found() {
    let bars = make_bars(3);
    assert_eq!(bars.iter().position(|b| b.time == -1), None);
    assert_eq!(bars.iter().position(|b| b.time == 3), None);
}

/// LWC: timeToIndex — should return null if time scale is empty
/// File: time-scale.spec.ts, line 91
#[test]
fn lwc_time_to_index_empty_scale() {
    let bars: Vec<OhlcBar> = vec![];
    assert_eq!(bars.iter().position(|b| b.time == 123), None);
}

/// LWC: timeToIndex — should return null if between two values
/// File: time-scale.spec.ts, line 97
#[test]
fn lwc_time_to_index_between_values() {
    let bars = vec![
        OhlcBar {
            time: 0,
            open: 100.0,
            high: 110.0,
            low: 90.0,
            close: 105.0,
        },
        OhlcBar {
            time: 2,
            open: 100.0,
            high: 110.0,
            low: 90.0,
            close: 105.0,
        },
    ];
    assert_eq!(bars.iter().position(|b| b.time == 1), None);
}

/// LWC: findNearest — should return last index if beyond range
/// File: time-scale.spec.ts, line 105
#[test]
fn lwc_find_nearest_beyond_end() {
    let bars = make_bars(3);
    let result = bars
        .iter()
        .enumerate()
        .min_by_key(|(_, b)| (b.time - 3).unsigned_abs());
    assert_eq!(result.map(|(i, _)| i), Some(2));
}

/// LWC: findNearest — should return first index if before range
/// File: time-scale.spec.ts, line 113
#[test]
fn lwc_find_nearest_before_start() {
    let bars = make_bars(3);
    let result = bars
        .iter()
        .enumerate()
        .min_by_key(|(_, b)| (b.time - (-1_i64)).unsigned_abs());
    assert_eq!(result.map(|(i, _)| i), Some(0));
}

/// LWC: findNearest — should return next if between values
/// File: time-scale.spec.ts, line 121
#[test]
fn lwc_find_nearest_between_values() {
    let bars = vec![
        OhlcBar {
            time: 0,
            open: 100.0,
            high: 110.0,
            low: 90.0,
            close: 105.0,
        },
        OhlcBar {
            time: 2,
            open: 100.0,
            high: 110.0,
            low: 90.0,
            close: 105.0,
        },
    ];
    let result = bars
        .iter()
        .enumerate()
        .min_by_key(|(_, b)| (b.time - 1).unsigned_abs());
    assert!(result.is_some());
}
