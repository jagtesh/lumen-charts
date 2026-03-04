//! Lumen Charts — GPU-accelerated charting library
//!
//! This crate is a facade that re-exports the core engine and the safe Rust SDK.
//!
//! # Usage
//!
//! ```ignore
//! use lumen_charts::sdk::{ChartApi, SeriesDefinition};
//! use lumen_charts::core::chart_model::OhlcBar;
//! ```

/// Core engine: chart state, rendering, C-ABI, backends.
pub use lumen_charts_core as core;

/// Safe, idiomatic Rust SDK (v5 compatible).
pub use lumen_charts_sdk as sdk;
