# Lumen Charts — SDK Reference

Platform-specific SDKs wrapping the core library with idiomatic, type-safe APIs.

| SDK | Language | Platforms | Entry Point |
|-----|----------|-----------|-------------|
| [`rust/`](rust/) | Rust | macOS, Linux, Windows | `ChartApi::with_renderer()` |
| [`swift/`](swift/) | Swift | macOS, iOS | `Chart(viewKind:viewHandle:...)` |
| [`wasm/`](wasm/) | JavaScript | Browser (WebGPU, Canvas 2D) | `createChart(element, options)` |

All SDKs implement the **v5 unified API** — a single `addSeries(type)` entry point
replaces the old per-type methods.

---

## Rust SDK

Safe, zero-unsafe wrapper around the core `Chart`. Provides `ChartApi`, `SeriesApi`,
`PaneApi`, `TimeScaleApi`, and `PriceScaleApi` handles.

```toml
# Cargo.toml
[dependencies]
lumen-charts-sdk = { path = "sdks/rust" }
lumen-charts = { path = "core" }  # needed for VelloRenderer
```

```rust
use lumen_charts::renderers::VelloRenderer;
use lumen_charts_sdk::{ChartApi, SeriesDefinition};

// Create chart with a Vello renderer
let renderer = VelloRenderer::new(instance, surface, device, queue, width, height, scale);
let mut chart = ChartApi::with_renderer(Box::new(renderer), width, height, scale);

// Load primary data
chart.set_data(ohlc_bars);
chart.fit_content();
chart.render();

// Add overlay series (v5 unified API)
let overlay = chart.add_series(SeriesDefinition::Area);
overlay.set_line_data(&mut chart, &line_points);

// Multi-pane MACD
let pane = chart.add_pane(0.3);
let hist = chart.add_series(SeriesDefinition::Histogram);
hist.set_histogram_data(&mut chart, &histogram_data);
hist.move_to_pane(&mut chart, &pane);

let macd_line = chart.add_series(SeriesDefinition::Line);
macd_line.set_line_data(&mut chart, &macd_data);
macd_line.apply_options(&mut chart, r#"{"color":[0.2,0.6,1.0,1.0]}"#);
macd_line.move_to_pane(&mut chart, &pane);

// Input handling — all methods return bool (true = needs redraw)
if chart.pointer_move(x, y) { chart.render(); }
if chart.scroll(dx, 0.0) { chart.render(); }
if chart.zoom(factor, center_x) { chart.render(); }
if chart.pinch(scale, cx, cy) { chart.render(); }
```

> See [`examples/rust-demo/`](../examples/rust-demo/) for a complete winit + wgpu + egui application.

---

## Swift SDK

Native Swift API for macOS and iOS. Wraps the C-ABI with type-safe handles.

```swift
import LumenCharts

// Create chart (Metal-backed)
let chart = Chart(
    viewKind: CHART_VIEW_METAL,
    viewHandle: metalLayerPtr,
    width: 900, height: 500,
    scaleFactor: 2.0
)

// Load primary data
chart.setData(ohlcBars)
chart.fitContent()
chart.render()

// Add overlay series (v5 unified API)
let overlay = chart.addSeries(.area)
overlay.setData(linePoints)

// Multi-pane MACD
let pane = chart.addPane(heightStretch: 0.3)

let hist = chart.addSeries(.histogram)
hist.setData(histogramData)
hist.moveToPane(pane)

let macdLine = chart.addSeries(.line)
macdLine.setData(macdData)
macdLine.applyOptions(LineSeriesOptions(
    color: ChartColor(r: 0.2, g: 0.6, b: 1.0),
    lineWidth: 1.5
))
macdLine.moveToPane(pane)

// Baseline series with custom base value
let baseline = chart.addSeries(.baseline(baseValue: 100.0))
baseline.setData(baselinePoints)

// Input handling
chart.pointerMove(x: x, y: y)
chart.scroll(deltaX: dx, deltaY: dy)
chart.zoom(factor: factor, centerX: cx)
chart.pinch(scale: scale, centerX: cx, centerY: cy)
```

> See [`examples/swift-demo/`](../examples/swift-demo/) for a complete macOS AppKit demo.

---

## JavaScript / WASM SDK

Browser API mirroring [Lightweight Charts](https://tradingview.github.io/lightweight-charts/).
Supports WebGPU (default) and Canvas 2D renderers.

```javascript
import { createChart } from './pkg/chart_api.js';

// WebGPU (default) or Canvas 2D
const chart = await createChart(document.getElementById('container'));
// const chart = await createChart(el, { renderer: 'canvas2d' });

// Load OHLC data
chart.setData([
    { time: 1704153600, open: 185.0, high: 187.5, low: 184.2, close: 186.3 },
    // ...
]);
chart.fitContent();

// Switch chart type (data stays the same)
chart.setSeriesType('candlestick');

// Add overlay series (v5 unified API)
const overlay = chart.addSeries('area', {
    lineColor: 'rgba(26, 153, 230, 1.0)',
    topColor: 'rgba(26, 153, 230, 0.4)',
    bottomColor: 'rgba(26, 153, 230, 0.0)',
});
overlay.setData([{ time: 1704153600, value: 186.3 }, /* ... */]);

// Multi-pane MACD
const pane = chart.addPane(0.3);

const hist = chart.addSeries('histogram', {});
hist.moveToPane(pane);
hist.setData([{ time: 1704153600, value: 0.5 }, /* ... */]);

const macdLine = chart.addSeries('line', {
    color: 'rgb(51, 153, 255)', lineWidth: 1.5
});
macdLine.moveToPane(pane);
macdLine.setData([{ time: 1704153600, value: 0.3 }, /* ... */]);

// Global options
chart.applyOptions({
    layout: { background: { color: '#1f1f1f' }, textColor: '#d1d4dc' },
    grid: { vertLines: { color: '#333' }, horzLines: { color: '#333' } }
});
```

> See [`examples/webgpu-demo/`](../examples/webgpu-demo/) and
> [`examples/web-canvas-demo/`](../examples/web-canvas-demo/) for complete browser demos.

---

## Shared Concepts

### Series Types

All SDKs support the same series types:

| Type | Rust | Swift | JS |
|------|------|-------|----|
| OHLC bars | `SeriesDefinition::Ohlc` | `.ohlc` | `'ohlc'` |
| Candlestick | `SeriesDefinition::Candlestick` | `.candlestick` | `'candlestick'` |
| Line | `SeriesDefinition::Line` | `.line` | `'line'` |
| Area | `SeriesDefinition::Area` | `.area` | `'area'` |
| Histogram | `SeriesDefinition::Histogram` | `.histogram` | `'histogram'` |
| Baseline | `SeriesDefinition::Baseline { base_value }` | `.baseline(baseValue:)` | `'baseline'` + `{ baseValue }` |

### Pane Management

```
chart.addPane(heightStretch)     → PaneApi / PaneHandle / PaneAPI
chart.removePane(pane)
chart.swapPanes(a, b)
series.moveToPane(pane)
series.getPane()                 → PaneApi / PaneHandle / PaneAPI
```

### Options (JSON-based)

All styling goes through JSON on the C-ABI boundary:

```
chart.applyOptions(json)
series.applyOptions(json)
timeScale.applyOptions(json)
priceScale.applyOptions(json)
```
