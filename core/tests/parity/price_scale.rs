/// Parity tests from LWC: tests/unittests/price-scale.spec.ts
use lumen_charts::chart_model::{OhlcBar, Rect};
use lumen_charts::price_scale::PriceScale;

fn plot_rect() -> Rect {
    Rect {
        x: 0.0,
        y: 0.0,
        width: 500.0,
        height: 500.0,
    }
}

/// LWC: visible range with normal mode — price roundtrip
/// File: price-scale.spec.ts, line 20
#[test]
fn lwc_price_scale_visible_range_normal() {
    let bars = vec![OhlcBar {
        time: 1,
        open: 0.0,
        high: 100_000.0,
        low: 0.0,
        close: 50_000.0,
    }];
    let ps = PriceScale::from_data(&bars);
    let plot = plot_rect();

    let y_top = ps.price_to_y(100_000.0, &plot);
    let y_bottom = ps.price_to_y(0.0, &plot);
    let price_top = ps.y_to_price(y_top, &plot);
    let price_bottom = ps.y_to_price(y_bottom, &plot);

    assert!((price_top - 100_000.0).abs() < 1.0);
    assert!((price_bottom - 0.0).abs() < 1.0);
}

/// LWC: logarithmic mode
/// File: price-scale.spec.ts, line 120
#[test]
#[ignore]
fn lwc_price_scale_visible_range_logarithmic() {
    todo!("Implement logarithmic price scale mode");
}
