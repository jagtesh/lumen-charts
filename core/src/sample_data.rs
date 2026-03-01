use crate::chart_model::OhlcBar;

/// Generate ~100 bars of realistic OHLCV data resembling AAPL daily prices
pub fn sample_data() -> Vec<OhlcBar> {
    // Start: Jan 2, 2024 — a plausible AAPL price series
    let base_time: i64 = 1704153600; // 2024-01-02 00:00:00 UTC
    let day: i64 = 86400;

    // Seed price and random walk
    let mut price: f64 = 185.0;
    let mut bars = Vec::with_capacity(100);

    // Deterministic pseudo-random: simple LCG
    let mut rng: u64 = 42;
    let next_rand = |rng: &mut u64| -> f64 {
        *rng = rng
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        // Map to [-1, 1]
        ((*rng >> 33) as f64 / (1u64 << 31) as f64) * 2.0 - 1.0
    };

    for i in 0..100 {
        let time = base_time + i * day;

        // Random daily change: up to ±2%
        let change_pct = next_rand(&mut rng) * 0.02;
        let daily_range = price * (0.005 + (next_rand(&mut rng).abs()) * 0.015);

        let open = price;
        let close = price * (1.0 + change_pct);
        let high = open.max(close) + daily_range * next_rand(&mut rng).abs();
        let low = open.min(close) - daily_range * next_rand(&mut rng).abs();

        bars.push(OhlcBar {
            time,
            open,
            high: high.max(open.max(close)),
            low: low.min(open.min(close)),
            close,
        });

        price = close;
    }

    bars
}
