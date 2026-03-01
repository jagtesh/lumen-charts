/// Parity tests from LWC: tests/unittests/price-scale.spec.ts
use lumen_charts::chart_model::{OhlcBar, Rect};
use lumen_charts::price_scale::{PriceScale, PriceScaleMode};

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

/// LWC: logarithmic mode — price roundtrip maintains relative proportions
/// File: price-scale.spec.ts, line 120
#[test]
fn lwc_price_scale_visible_range_logarithmic() {
    let bars = vec![OhlcBar {
        time: 1,
        open: 10.0,
        high: 10_000.0,
        low: 10.0,
        close: 5_000.0,
    }];
    let mut ps = PriceScale::from_data(&bars);
    ps.mode = PriceScaleMode::Logarithmic;
    let plot = plot_rect();

    // In log mode, prices should roundtrip correctly
    let test_prices = [10.0, 100.0, 1000.0, 5000.0, 10_000.0];
    for &price in &test_prices {
        let y = ps.price_to_y(price, &plot);
        let back = ps.y_to_price(y, &plot);
        assert!(
            (back - price).abs() / price < 0.01,
            "Log roundtrip failed for {}: got {:.2}",
            price,
            back
        );
    }

    // Key log property: equal ratios should map to equal pixel distances
    // Distance from 10→100 should equal distance from 100→1000 (each is 10×)
    let y10 = ps.price_to_y(10.0, &plot);
    let y100 = ps.price_to_y(100.0, &plot);
    let y1000 = ps.price_to_y(1000.0, &plot);
    let y10000 = ps.price_to_y(10_000.0, &plot);

    let dist_10_100 = (y10 - y100).abs();
    let dist_100_1000 = (y100 - y1000).abs();
    let dist_1000_10000 = (y1000 - y10000).abs();

    assert!(
        (dist_10_100 - dist_100_1000).abs() < 2.0,
        "Log distances should be equal: {} vs {}",
        dist_10_100,
        dist_100_1000
    );
    assert!(
        (dist_100_1000 - dist_1000_10000).abs() < 2.0,
        "Log distances should be equal: {} vs {}",
        dist_100_1000,
        dist_1000_10000
    );
}
