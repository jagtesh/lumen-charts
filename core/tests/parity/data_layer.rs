/// Parity tests from LWC: tests/unittests/data-layer.spec.ts
use lumen_charts_core::chart_model::OhlcBar;
use lumen_charts_core::data_layer::DataLayer;

fn bar_at(time: i64) -> OhlcBar {
    OhlcBar {
        time,
        open: 0.0,
        high: 0.0,
        low: 0.0,
        close: 0.0,
    }
}

fn changed_bar_at(time: i64) -> OhlcBar {
    OhlcBar {
        time,
        open: 1.0,
        high: 1.0,
        low: 1.0,
        close: 1.0,
    }
}

/// LWC: should be able to add new point in the end
/// File: data-layer.spec.ts, line 211
#[test]
fn lwc_add_point_at_end() {
    let mut dl = DataLayer::from_bars(vec![bar_at(1000), bar_at(3000)]);
    dl.update(bar_at(5000));
    assert_eq!(dl.len(), 3);
    assert_eq!(dl.bars()[2].time, 5000);
}

/// LWC: should be able to change last existing point
/// File: data-layer.spec.ts, line 267
#[test]
fn lwc_change_last_point() {
    let mut dl = DataLayer::from_bars(vec![bar_at(1000), bar_at(4000)]);
    dl.update(changed_bar_at(4000));
    assert_eq!(dl.len(), 2);
    assert_eq!(dl.bars()[1].close, 1.0);
}

/// LWC: should be able to change an historical point
/// File: data-layer.spec.ts, line 306
#[test]
fn lwc_change_historical_point() {
    let mut dl = DataLayer::from_bars(vec![
        bar_at(1000),
        bar_at(3000),
        bar_at(4000),
        bar_at(6000),
        bar_at(8000),
    ]);
    dl.update(changed_bar_at(3000));
    assert_eq!(dl.len(), 5);
    assert_eq!(dl.bars()[1].close, 1.0);
    assert_eq!(dl.bars()[0].close, 0.0);
    assert_eq!(dl.bars()[2].close, 0.0);
}

/// LWC: should be able to add new point in the middle
/// File: data-layer.spec.ts, line 408
#[test]
fn lwc_add_point_in_middle() {
    let mut dl = DataLayer::from_bars(vec![bar_at(2000), bar_at(5000)]);
    dl.update(bar_at(3000));
    assert_eq!(dl.len(), 3);
    assert_eq!(dl.bars()[0].time, 2000);
    assert_eq!(dl.bars()[1].time, 3000);
    assert_eq!(dl.bars()[2].time, 5000);
}

/// LWC: should correctly update indexes if times unchanged
/// File: data-layer.spec.ts, line 608
#[test]
fn lwc_update_indexes_same_times() {
    let mut dl = DataLayer::from_bars(vec![bar_at(1000), bar_at(3000)]);
    dl.set_data(vec![bar_at(1000), bar_at(3000)]);
    assert_eq!(dl.len(), 2);
    assert_eq!(dl.bars()[0].time, 1000);
    assert_eq!(dl.bars()[1].time, 3000);
}

/// LWC: base index null when data cleared
/// File: data-layer.spec.ts, line 674
#[test]
fn lwc_clear_data_empty() {
    let mut dl = DataLayer::from_bars(vec![
        OhlcBar {
            time: 1609459200,
            open: 0.0,
            high: 0.0,
            low: 0.0,
            close: 31.53,
        },
        OhlcBar {
            time: 1609545600,
            open: 0.0,
            high: 0.0,
            low: 0.0,
            close: 6.57,
        },
    ]);
    dl.set_data(vec![]);
    assert_eq!(dl.len(), 0);
}

/// LWC: should pop data from series
/// File: data-layer.spec.ts, Series Popping
#[test]
fn lwc_pop_data() {
    let mut dl = DataLayer::from_bars(vec![bar_at(1000), bar_at(2000), bar_at(3000)]);
    let popped = dl.pop();
    assert!(popped.is_some());
    assert_eq!(popped.unwrap().time, 3000);
    assert_eq!(dl.len(), 2);
}

/// LWC: OHLC ignores "value" field — enforced by type system
/// File: data-layer.spec.ts, line 527
#[test]
fn lwc_ohlc_ignores_value_field() {
    let bar = OhlcBar {
        time: 1000,
        open: 10.0,
        high: 15.0,
        low: 5.0,
        close: 11.0,
    };
    let dl = DataLayer::from_bars(vec![bar]);
    assert_eq!(dl.bars()[0].open, 10.0);
    assert_eq!(dl.bars()[0].high, 15.0);
    assert_eq!(dl.bars()[0].low, 5.0);
    assert_eq!(dl.bars()[0].close, 11.0);
}
