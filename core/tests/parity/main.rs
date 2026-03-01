// LWC Parity Tests — Integration test entry point
// =================================================
// Ported from tradingview/lightweight-charts unit test suite.
// Each module mirrors a corresponding LWC .spec.ts file.
//
// Tests marked #[ignore] require features not yet implemented (TDD targets).
// As features land, remove #[ignore] and the test should pass.
//
// Run: cargo test --test parity
// Run all (including ignored): cargo test --test parity -- --include-ignored

mod color;
mod data_layer;
mod formatters;
mod plot_list;
mod price_scale;
mod series;
mod tick_mark_formatter;
mod time_scale;
mod timed_data;
