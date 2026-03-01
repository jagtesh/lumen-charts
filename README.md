# Lumen Charts

GPU-accelerated charting library built on [Vello](https://github.com/linebender/vello), inspired by [Lightweight Charts](https://github.com/nicjbrow/lightweight-charts).

## Project Structure

```
├── core/               Rust core library (rendering, state, C-ABI)
│   ├── src/            Source code
│   ├── include/        C header (chart_core.h)
│   └── target/         Build output (cargo build --release)
├── sdks/
│   ├── swift/          Swift wrapper (LightweightCharts module)
│   └── wasm/           WebAssembly bindings + JS API wrapper
│       ├── src/        wasm-bindgen zero-cost passthrough to C-ABI
│       └── chart_api.js  Lightweight Charts–style JS API
└── examples/
    ├── swift-demo/     macOS demo app (SwiftUI + Metal)
    └── web-demo/       Browser demo (HTML + WebGPU)
```

## Quick Start

### Build the Core Library

```bash
cd core && cargo build --release
```

### Run the Swift Demo

```bash
cd core && cargo build --release
cd ../examples/swift-demo && swift run
```

### Run the WASM / WebGPU Demo

```bash
cd examples/web-demo && ./run.sh
# Opens at http://localhost:8080 (Chrome 113+ or Safari 18+ for WebGPU)
```

The build script compiles the Rust core to WebAssembly via `wasm-pack`, copies the
JS API wrapper into the output `pkg/` directory, and starts a local HTTP server.

### Run Tests

```bash
cd core && cargo test
```

## Architecture

The **core** is a platform-agnostic Rust library that exposes a C-ABI. It handles:
- OHLC, Candlestick, Line, Area, Baseline, Histogram series
- Time scale with zoom, scroll, and fit-to-content
- Price scale with auto-range and percentage mode
- Multi-pane layout with add/remove/reorder
- Crosshair, price lines, and event system
- Invalidation-driven rendering (only redraws when state changes)
- GPU rendering via Vello + wgpu

**SDKs** wrap the C-ABI with idiomatic, type-safe APIs for each platform:
- **Swift SDK** — native Swift classes wrapping the C-ABI, with MetalLayer integration
- **WASM SDK** — `wasm-bindgen` zero-cost passthrough + `chart_api.js` wrapper

**Examples** are runnable demos that showcase the SDK usage.

## JavaScript API (WASM)

The WASM SDK includes `chart_api.js`, a JavaScript wrapper that mirrors the
[Lightweight Charts](https://www.tradingview.com/lightweight-charts/) API:

```javascript
import { createChart } from './pkg/chart_api.js';

const chart = await createChart(document.getElementById('container'));

// Load OHLC data
chart.setData([
    { time: 1704153600, open: 185.0, high: 187.5, low: 184.2, close: 186.3 },
    // ...
]);

// Switch rendering type (data stays the same)
chart.setSeriesType('candlestick');  // 'ohlc' | 'candlestick' | 'line' | 'area' | 'histogram' | 'baseline'

// Add overlay series
const overlay = chart.addAreaSeries({ lineColor: '#2962FF' });
overlay.setData([{ time: 1704153600, value: 186.3 }, /* ... */]);

// Multi-pane support
const macdPane = chart.addPane(0.3);
const histSeries = chart.addHistogramSeries({});
histSeries.moveToPane(macdPane);
histSeries.setData([{ time: 1704153600, value: 0.5 }, /* ... */]);

// Global options
chart.applyOptions({
    layout: { background: { color: '#1f1f1f' }, textColor: '#d1d4dc' },
    grid: { vertLines: { color: '#333' }, horzLines: { color: '#333' } }
});

chart.fitContent();
```

### Data Validation

The JS API validates all input data at the boundary:
- **Throws `TypeError`** on missing required fields (`time`, `open`/`high`/`low`/`close` for OHLC, `value` for line)
- **Warns** when suboptimal data is passed (e.g., OHLC data with extra `value` field)
- **Auto-converts** `close` → `value` for line/area series with a console warning

## Swift Demo Features

Both the Swift and WASM demos support:
- **Chart Type Selector** — OHLC, Candlestick, Line, Area, Histogram, Baseline
- **Fit Content** — auto-zoom to show all data
- **Toggle Overlay** — add/remove an Area series on the main pane
- **Toggle MACD** — add/remove a MACD indicator (histogram + 2 lines) in a separate pane

## License

Apache License 2.0 — see [LICENSE](LICENSE) and [NOTICE](NOTICE).
