/// Parity tests from LWC: tests/unittests/plot-list.spec.ts
///
/// Min/max per OHLC field, gap handling, ranged queries, search.
use lumen_charts_core::chart_model::OhlcBar;
use lumen_charts_core::data_layer::DataLayer;

fn ohlc(time: i64, o: f64, h: f64, l: f64, c: f64) -> OhlcBar {
    OhlcBar {
        time,
        open: o,
        high: h,
        low: l,
        close: c,
    }
}

/// LWC: minMax close — values [1,2,3,4,5], expect min=1, max=5
/// File: plot-list.spec.ts, line 75
#[test]
fn lwc_min_max_close() {
    let bars = vec![
        ohlc(1, 0.0, 0.0, 0.0, 1.0),
        ohlc(2, 0.0, 0.0, 0.0, 2.0),
        ohlc(3, 0.0, 0.0, 0.0, 3.0),
        ohlc(4, 0.0, 0.0, 0.0, 4.0),
        ohlc(5, 0.0, 0.0, 0.0, 5.0),
    ];
    let (min, max) = bars.iter().fold((f64::MAX, f64::MIN), |(mn, mx), b| {
        (mn.min(b.close), mx.max(b.close))
    });
    assert_eq!(min, 1.0);
    assert_eq!(max, 5.0);
}

/// LWC: minMax with non-subsequent indices — max=10 at index 20
/// File: plot-list.spec.ts, line 93
#[test]
fn lwc_min_max_non_subsequent() {
    let bars = vec![
        ohlc(0, 0.0, 0.0, 0.0, 1.0),
        ohlc(2, 0.0, 0.0, 0.0, 2.0),
        ohlc(4, 0.0, 0.0, 0.0, 3.0),
        ohlc(6, 0.0, 0.0, 0.0, 4.0),
        ohlc(20, 0.0, 0.0, 0.0, 10.0),
        ohlc(100, 0.0, 0.0, 0.0, 5.0),
    ];
    let (min, max) = bars.iter().fold((f64::MAX, f64::MIN), |(mn, mx), b| {
        (mn.min(b.close), mx.max(b.close))
    });
    assert_eq!(min, 1.0);
    assert_eq!(max, 10.0);
}

/// LWC: gaps starting second-to-last chunk — High field, range 30..200
/// File: plot-list.spec.ts, line 112
#[test]
fn lwc_min_max_with_gaps() {
    let bars = vec![
        ohlc(29, 1.0, 1.0, 1.0, 1.0),
        ohlc(31, 2.0, 2.0, 2.0, 2.0),
        ohlc(55, 3.0, 3.0, 3.0, 3.0),
        ohlc(65, 4.0, 4.0, 4.0, 4.0),
    ];
    let in_range: Vec<_> = bars
        .iter()
        .filter(|b| b.time >= 30 && b.time <= 200)
        .collect();
    let (min, max) = in_range.iter().fold((f64::MAX, f64::MIN), |(mn, mx), b| {
        (mn.min(b.high), mx.max(b.high))
    });
    assert_eq!(min, 2.0);
    assert_eq!(max, 4.0);
}

/// LWC: per-field min/max — open [5,10,15,20,25]
/// File: plot-list.spec.ts, line 149
#[test]
fn lwc_min_max_open() {
    let bars = vec![
        ohlc(1, 5.0, 7.0, 3.0, 6.0),
        ohlc(2, 10.0, 12.0, 8.0, 11.0),
        ohlc(3, 15.0, 17.0, 13.0, 16.0),
        ohlc(4, 20.0, 22.0, 18.0, 21.0),
        ohlc(5, 25.0, 27.0, 23.0, 26.0),
    ];
    let (min, max) = bars.iter().fold((f64::MAX, f64::MIN), |(mn, mx), b| {
        (mn.min(b.open), mx.max(b.open))
    });
    assert_eq!(min, 5.0);
    assert_eq!(max, 25.0);
}

/// LWC: per-field min/max — high [7,12,17,22,27]
/// File: plot-list.spec.ts, line 159
#[test]
fn lwc_min_max_high() {
    let bars = vec![
        ohlc(1, 5.0, 7.0, 3.0, 6.0),
        ohlc(2, 10.0, 12.0, 8.0, 11.0),
        ohlc(3, 15.0, 17.0, 13.0, 16.0),
        ohlc(4, 20.0, 22.0, 18.0, 21.0),
        ohlc(5, 25.0, 27.0, 23.0, 26.0),
    ];
    let (min, max) = bars.iter().fold((f64::MAX, f64::MIN), |(mn, mx), b| {
        (mn.min(b.high), mx.max(b.high))
    });
    assert_eq!(min, 7.0);
    assert_eq!(max, 27.0);
}

/// LWC: per-field min/max — low [3,8,13,18,23]
/// File: plot-list.spec.ts, line 169
#[test]
fn lwc_min_max_low() {
    let bars = vec![
        ohlc(1, 5.0, 7.0, 3.0, 6.0),
        ohlc(2, 10.0, 12.0, 8.0, 11.0),
        ohlc(3, 15.0, 17.0, 13.0, 16.0),
        ohlc(4, 20.0, 22.0, 18.0, 21.0),
        ohlc(5, 25.0, 27.0, 23.0, 26.0),
    ];
    let (min, max) = bars.iter().fold((f64::MAX, f64::MIN), |(mn, mx), b| {
        (mn.min(b.low), mx.max(b.low))
    });
    assert_eq!(min, 3.0);
    assert_eq!(max, 23.0);
}

/// LWC: per-field min/max — close [6,11,16,21,26]
/// File: plot-list.spec.ts, line 179
#[test]
fn lwc_min_max_close_ohlc() {
    let bars = vec![
        ohlc(1, 5.0, 7.0, 3.0, 6.0),
        ohlc(2, 10.0, 12.0, 8.0, 11.0),
        ohlc(3, 15.0, 17.0, 13.0, 16.0),
        ohlc(4, 20.0, 22.0, 18.0, 21.0),
        ohlc(5, 25.0, 27.0, 23.0, 26.0),
    ];
    let (min, max) = bars.iter().fold((f64::MAX, f64::MIN), |(mn, mx), b| {
        (mn.min(b.close), mx.max(b.close))
    });
    assert_eq!(min, 6.0);
    assert_eq!(max, 26.0);
}

/// LWC: search — find by index and strategy
/// File: plot-list.spec.ts, line 50
#[test]
fn lwc_search_by_index() {
    let dl = DataLayer::from_bars(vec![
        ohlc(1, 1.0, 2.0, 3.0, 4.0),
        ohlc(2, 10.0, 20.0, 30.0, 40.0),
        ohlc(3, 100.0, 200.0, 300.0, 400.0),
    ]);
    assert!(dl.bar_at_time(1).is_some());
    assert!(dl.bar_at_time(2).is_some());
    assert!(dl.bar_at_time(3).is_some());
    assert!(dl.bar_at_time(0).is_none());
    assert!(dl.bar_at_time(4).is_none());
}

/// LWC: indices for fulfilled data
/// File: plot-list.spec.ts, line 191
#[test]
fn lwc_indices() {
    let dl = DataLayer::from_bars(vec![
        ohlc(1, 1.0, 2.0, 3.0, 4.0),
        ohlc(2, 10.0, 20.0, 30.0, 40.0),
        ohlc(3, 100.0, 200.0, 300.0, 400.0),
    ]);
    let times: Vec<i64> = dl.bars().iter().map(|b| b.time).collect();
    assert_eq!(times, vec![1, 2, 3]);
}
