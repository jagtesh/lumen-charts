# Lightweight Charts — Rust Core

GPU-accelerated charting library built on [Vello](https://github.com/linebender/vello), with API parity to [Lightweight Charts](https://github.com/nicjbrow/lightweight-charts).

## Project Structure

```
├── core/               Rust core library (rendering, state, C-ABI)
├── sdks/
│   ├── swift/          Swift wrapper (LightweightCharts module)
│   └── wasm/           WebAssembly bindings (wasm-bindgen)
└── examples/
    ├── swift-demo/     macOS demo app
    └── web-demo/       Browser demo (HTML + WebGPU)
```

## Quick Start

### Build the core library

```bash
cd core && cargo build --release
```

### Run the Swift demo

```bash
cd core && cargo build --release
cd ../examples/swift-demo && swift run
```

### Run tests

```bash
cd core && cargo test
```

## Architecture

The **core** is a platform-agnostic Rust library that exposes a C-ABI. It handles:
- OHLC, Candlestick, Line, Area, Baseline, Histogram series
- Time scale with zoom/scroll/fit
- Price scale with auto-range
- Multi-pane layout
- Crosshair, price lines, event system
- GPU rendering via Vello + wgpu

**SDKs** wrap the C-ABI with idiomatic, type-safe APIs for each platform.

**Examples** are runnable demos that showcase the SDK usage.
