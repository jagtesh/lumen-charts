# LWC v5 Parity Gap Analysis
**Date:** 2026-03-04  
**Compared Against:** TradingView Lightweight Charts **v5** API  
**Source:** [API docs](https://tradingview.github.io/lightweight-charts/docs/api) + [GitHub](https://github.com/tradingview/lightweight-charts)

---

## Delta vs Previous Audit ([lwc-parity.md](file:///Users/jagtesh/.gemini/antigravity/playground/chart-mvp/assets/lwc-parity.md))

Previous audit (2026-03-01) compared against **LWC v4** claiming **94% coverage** (48/51).

### v5 APIs not in v4

| New v5 Feature | Status |
|----------------|--------|
| `IHorzScaleBehavior` — custom X-axis | 🅿️ Parked |
| `addCustomSeries()` + `ICustomSeriesPaneView` | 🅿️ Parked |
| `ISeriesPrimitive` / `IPanePrimitive` | 🅿️ Parked |
| `addPane` / `removePane` / `swapPanes` | ✅ Already have |
| `moveToPane` | ✅ Already have |
| `seriesOrder` / `setSeriesOrder` | ⚠️ In C-ABI wrapper, needs refactor |
| `getPane` | ⚠️ In C-ABI wrapper, needs refactor |

### Gaps closed this session

| Item | Commit |
|------|--------|
| Per-pane price auto-fit | `46d49c0` |
| Unified time axis | `46d49c0` |
| FFI `pane_index` across SDKs | `7b5e2eb` |
| Whitespace gap rendering | `0c0b534` |
| 11 new regression tests | `dd9fd63`, `0c0b534`, `053b2a6` |

---

## C-ABI Wrapper Misplacement

> [!WARNING]
> These features have business logic implemented directly in `lib.rs` (C-ABI wrapper) instead of proper structs. The wrapper should be a bare pass-through. Logic needs to move to the correct struct methods.

| C-ABI Function | Current Location | Should Be In |
|----------------|-----------------|--------------|
| `chart_series_order()` | `lib.rs:1854` — iterates series, computes z-order | `SeriesCollection` or `ChartState` method |
| `chart_series_set_order()` | `lib.rs:1877` — reorders series vec | `SeriesCollection` or `ChartState` method |
| `chart_series_get_pane_index()` | `lib.rs:1839` — reads `series.pane_index` | `SeriesCollection::get_pane_index()` |
| `chart_series_pop()` | `lib.rs:957` — calls `data.pop()` + `series_data_changed()` | `ChartState::series_pop()` |
| `chart_format_price()` | `lib.rs:2367` — format price | `ChartState` or `Formatters` method |
| `chart_format_date()` | `lib.rs:2381` — format date | Same |
| `chart_format_time()` | `lib.rs:2401` — format time | Same |

---

## Approved for Implementation (ordered by severity + relatedness)

### Group 1: autoSizeActive / Disable Auto-fit
**Problem:** After resizing the Y-axis manually, click-dragging auto-resizes back to fit all data. No way to disable auto-fit.
- Add `auto_scale: bool` flag to `PriceScale`
- After Y-axis manual drag, set `auto_scale = false`
- Expose `set_auto_scale(pane_index, enabled)` via FFI
- Default: `true` (current behavior)

### Group 2: AutoscaleInfoProvider
Per-series autoscale override — allows a series to customize its contribution to autoscale (e.g., margins, excluded ranges).

### Group 3: Line Style Variants
Add missing `LineStyle` variants:
- `Dotted`
- `LargeDashed`
- `SparseDotted`

### Group 4: LineType (Interpolation)
Add `LineType` enum:
- `Simple` (current — straight segments) ✅
- `WithSteps` — horizontal then vertical (staircase)
- `Curved` — cubic Bézier / Catmull-Rom spline

### Group 5: LastPriceAnimationMode
Animated circle/pulse on the last price point:
- `Disabled` (default)
- `Continuous` — always pulsing
- `OnDataUpdate` — pulse when new data arrives

### Group 6: CrosshairMode Magnet
Snap crosshair to nearest data point instead of free-floating:
- `Normal` ✅ (current)
- `Magnet` — snap Y to closest OHLC value
- `Hidden` — no crosshair line

### Group 7: C-ABI Wrapper Refactor
Move logic from `lib.rs` wrapper into proper struct methods (see table above).

---

## Parked

| Feature | Reason |
|---------|--------|
| Custom horizontal scale (`IHorzScaleBehavior`) | Future extensibility |
| Custom series (`addCustomSeries`) | Doesn't help MACD — see note below |
| Primitives & plugins | Future extensibility |
| `takeScreenshot()` | GPU readback complexity |
| `invertScale` / `scaleMargins` | Low priority |
| Percentage price scale mode | Low priority |
| IndexedTo100 price scale mode | Low priority — see note below |
| Per-series independent price scales | Future |

---

## Answered Questions

### IndexedTo100 vs Percentage — what's the difference?

Both normalize prices relative to the first visible bar, but display differently:

| Mode | First visible bar = $100 | Current bar = $110 | Current bar = $90 |
|------|--------------------------|--------------------|--------------------|
| **Percentage** | 0% | +10% | -10% |
| **IndexedTo100** | 100 | 110 | 90 |

Percentage shows change as a percentage. IndexedTo100 rebases to 100 and shows absolute rebased values. Both parked per your request.

### Custom series — does it help MACD?

**No.** Custom series (`ICustomSeriesPaneView`) is for entirely novel chart types (heatmaps, renko, point-and-figure) where you provide your own renderer. MACD uses standard **Line** (signal) + **Histogram** series which we already support natively. Parked.
