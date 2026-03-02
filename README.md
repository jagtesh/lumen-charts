# Lumen Charts

GPU-accelerated charting library built on [Vello](https://github.com/linebender/vello), inspired by [Lightweight Charts](https://github.com/tradingview/lightweight-charts).

The API is designed to stay as close to the original Lightweight Charts API as
possible, making migration straightforward:

- **Swift SDK** — native API for macOS and iOS (via Metal)
- **JavaScript API** — available for the WASM target, requires WebGPU support in
  the browser (Chrome 113+, Safari 18+)
- **WebGL / Canvas** — planned. Note: a Canvas 2D target may not be worthwhile
  since you could use Lightweight Charts natively at that point
- **Windows / Linux** — no high-level SDK yet; use the C-ABI directly
  (see [Platform Support](#platform-support))

### Swift Demo (macOS, Metal)

![Swift Demo — Candlestick + Overlay + MACD](https://raw.githubusercontent.com/jagtesh/lumen-charts/main/assets/swift-demo.png)

### WASM Demo (Chrome, WebGPU)

![WASM Web Demo — Candlestick + MACD](https://raw.githubusercontent.com/jagtesh/lumen-charts/main/assets/web-demo.png)

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

## Installation

### Rust (crates.io)

```toml
[dependencies]
lumen-charts = "1.1"
```

### Swift (Swift Package Manager)

Add Lumen Charts as a dependency in your `Package.swift`:

```swift
dependencies: [
    .package(url: "https://github.com/jagtesh/lumen-charts.git", from: "1.1.0"),
],
targets: [
    .target(
        name: "YourApp",
        dependencies: [
            .product(name: "LightweightCharts", package: "lumen-charts"),
        ]
    ),
]
```

> **Note:** The Swift SDK wraps the C-ABI via `CChartCore`. You must first build the
> native library (`cd core && cargo build --release`) before `swift build` can link it.

### WASM / JavaScript

The WASM SDK requires building from source via `wasm-pack`:

```bash
git clone https://github.com/jagtesh/lumen-charts.git
cd lumen-charts/sdks/wasm
wasm-pack build --target web
```

This produces a `pkg/` directory you can import in your JavaScript:

```javascript
import { createChart } from './pkg/chart_api.js';
const chart = await createChart(document.getElementById('container'));
```

> Requires a browser with WebGPU support (Chrome 113+, Safari 18+).

### C / C++ / Other Languages

Use the C-ABI directly via the header file. Build the static library and link it:

```bash
cd core && cargo build --release
# Link against target/release/liblumen_charts.a (or .dylib / .dll)
# Include core/include/chart_core.h
```

## Quick Start

```bash
git clone https://github.com/jagtesh/lumen-charts.git
cd lumen-charts
```

### Build the Core Library

```bash
make core-libs    # builds core/target/release/liblumen_charts.a
```

### Run the Swift Demo

```bash
make swift-demo   # core-libs → sync header → run Swift demo
```

> Override the library path with `LUMEN_LIB_PATH=/custom/path make swift-demo`

### Run the WASM / WebGPU Demo

```bash
make wasm-demo    # builds WASM SDK → starts local server at http://localhost:8080
```

> Requires Chrome 113+ or Safari 18+ for WebGPU.

### Run Tests

```bash
make test         # runs all 265 tests (unit + integration + parity + C-ABI callback)
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

## API Completeness

Lumen Charts targets **full parity** with [Lightweight Charts v4](https://tradingview.github.io/lightweight-charts/). Current coverage:

| Interface | Coverage | Notes |
|---|---|---|
| `IChartApi` | 80% | `addCustomSeries` and `takeScreenshot` deferred |
| `ISeriesApi` | **100%** ✅ | All methods including `setMarkers`, `barsInLogicalRange` |
| `ITimeScaleApi` | **100%** ✅ | All methods including event subscriptions |
| `IPriceScaleApi` | **100%** ✅ | All methods including `applyOptions`, `width`, `options` |
| **Overall** | **94%** | [Full audit →](assets/lwc-parity.md) |

All options use **JSON** across the C-ABI boundary — the same `applyOptions(json)` / `options() → json` pattern works everywhere (chart, series, time scale, price scale).

### Beyond Lightweight Charts

Lumen Charts also provides features **not available** in Lightweight Charts:

- **Multi-pane layout** — `addPane`, `removePane`, `swapPanes`, `moveToPane`
- **Touch gesture recognition** — tap, long-press, pan, pinch-to-zoom
- **Invalidation-driven rendering** — `renderIfNeeded()` skips GPU work when nothing changed
- **Keyboard navigation** — arrow keys, +/−, Home/End
- **Series pop** — efficiently remove the last N data points

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

## Platform Support

The core uses `wgpu` with automatic backend selection (`Backends::all()`). No
configuration is needed — the best GPU API is chosen at runtime:

| Platform       | GPU Backend | Surface Source           | SDK                        |
|----------------|-------------|--------------------------|----------------------------|
| macOS / iOS    | Metal       | `CAMetalLayer`           | ✅ Swift SDK               |
| Browser (WASM) | WebGPU      | `<canvas>` element       | ✅ JavaScript API          |
| Windows        | DX12/Vulkan | `HWND`                   | C-ABI only (no SDK yet)    |
| Linux          | Vulkan      | Wayland/X11 surface      | C-ABI only (no SDK yet)    |
| Android        | Vulkan      | `ANativeWindow`          | C-ABI only (no SDK yet)    |

For platforms without a dedicated SDK (Windows, Linux, Android), you work with
the low-level C-ABI directly via `chart_core.h`. The full C header is at
`core/include/chart_core.h`. All `chart_*` functions are platform-agnostic — only
the initial surface creation call differs per platform.

### Windows (Win32 / HWND)

To embed a chart in a Win32 application, you'd create a `chart_create` variant
that accepts an `HWND`. The core already links against DX12/Vulkan automatically
— only the surface creation entry point needs to be platform-specific:

```c
#include "chart_core.h"
#include <windows.h>

// Hypothetical entry point (not yet implemented):
// Chart* chart_create_win32(HWND hwnd, uint32_t width, uint32_t height, float scale);

LRESULT CALLBACK WndProc(HWND hwnd, UINT msg, WPARAM wp, LPARAM lp) {
    static Chart* chart = NULL;
    switch (msg) {
        case WM_CREATE:
            chart = chart_create_win32(hwnd, 900, 500, 1.0f);
            // Load data, set options...
            break;
        case WM_PAINT:
            chart_render_if_needed(chart);
            break;
        case WM_MOUSEMOVE:
            chart_pointer_move(chart, GET_X_LPARAM(lp), GET_Y_LPARAM(lp));
            break;
    }
    return DefWindowProc(hwnd, msg, wp, lp);
}
```

### Linux (GTK4 + GDK / Wayland)

For GTK4 apps, you'd obtain the native Wayland or X11 surface from GDK and pass
it to a Linux-specific `chart_create` variant:

```c
#include "chart_core.h"
#include <gtk/gtk.h>

// Hypothetical entry point (not yet implemented):
// Chart* chart_create_wayland(void* wl_surface, uint32_t w, uint32_t h, float scale);
// Chart* chart_create_x11(uint32_t window_id, uint32_t w, uint32_t h, float scale);

static void on_realize(GtkWidget *widget, gpointer data) {
    GdkSurface *gdk_surface = gtk_native_get_surface(GTK_NATIVE(widget));

    // For Wayland:
    struct wl_surface *wl = gdk_wayland_surface_get_wl_surface(gdk_surface);
    Chart *chart = chart_create_wayland(wl, 900, 500, 1.0f);

    // All subsequent API calls are the same across all platforms:
    // chart_set_data(chart, data, len);
    // chart_fit_content(chart);
    // chart_pointer_move(chart, x, y);  etc.
}
```

> **Note:** The rendering pipeline (Vello → wgpu → GPU) and the entire C-ABI are
> identical across all platforms. Only the surface creation function differs per
> platform. All `chart_*` functions work the same everywhere once the `Chart*` is
> created.

## License

Apache License 2.0 — see [LICENSE](LICENSE) and [NOTICE](NOTICE).
