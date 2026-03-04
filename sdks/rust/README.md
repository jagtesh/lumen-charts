# Lumen Charts — Rust SDK

The Rust SDK source has moved to the workspace root as a first-class crate.

**Source location:** [`../../rust-sdk/`](../../rust-sdk/)

## Usage

The SDK is re-exported through the `lumen-charts` facade crate:

```rust
use lumen_charts::sdk::{ChartApi, SeriesDefinition, OhlcBar};
use lumen_charts::core::chart_model::OhlcBar;
```

See the [rust-demo](../../examples/rust-demo/) for a complete example.
