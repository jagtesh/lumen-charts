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
| `seriesOrder` / `setSeriesOrder` | ✅ Done — `SeriesCollection` methods (`092595d`) |
| `getPane` | ✅ Done — `SeriesCollection::get_pane_index()` (`092595d`) |

### Gaps closed this session

| Item | Commit |
|------|--------|
| Per-pane price auto-fit | `46d49c0` |
| Unified time axis | `46d49c0` |
| FFI `pane_index` across SDKs | `7b5e2eb` |
| Whitespace gap rendering | `0c0b534` |
| 11 new regression tests | `dd9fd63`, `0c0b534`, `053b2a6` |

---

## C-ABI Wrapper Refactor — ✅ Complete

> [!NOTE]
> All business logic has been moved from `lib.rs` to proper structs. The wrapper is now a bare pass-through.

| C-ABI Function | Moved To | Commit |
|----------------|----------|--------|
| `chart_series_order()` | `SeriesCollection::series_order()` | `092595d` |
| `chart_series_set_order()` | `SeriesCollection::set_series_order()` | `092595d` |
| `chart_series_get_pane_index()` | `SeriesCollection::get_pane_index()` | `092595d` |
| `chart_series_pop()` | `SeriesCollection::pop_series()` | `092595d` |
| `chart_series_set_markers()` | `Overlays::set_markers_from_json()` | `c5e5720` |
| `chart_series_markers()` | `Overlays::markers_to_json()` | `c5e5720` |
| `chart_set_series_type()` | `ChartState::set_series_type()` | `c5e5720` |
| `chart_series_get_options()` | `Series::options_json()` | `c5e5720` |
| `chart_format_price/date/time()` | Already thin — delegates to `Formatters` | — |

---

## Approved for Implementation (ordered by severity + relatedness)

### Group 1: autoSizeActive / Disable Auto-fit ✅ `d0c3248`
**Done:** `auto_scale: bool` on `PriceScale` (default true). Guard in `update_price_scale()`. FFI `chart_price_scale_get/set_auto_scale`. Test: `test_auto_scale_false_locks_price_range`.
- ~~Add `auto_scale: bool` flag to `PriceScale`~~
- ~~After Y-axis manual drag, set `auto_scale = false`~~
- ~~Expose `set_auto_scale(pane_index, enabled)` via FFI~~
- ~~Default: `true` (current behavior)~~

> [!NOTE]
> Auto-scale disable on manual Y-axis drag is left as a client-side decision per user preference — the option is exposed, not forced.

### Group 2: AutoscaleInfoProvider ⏳
Per-series autoscale override — allows a series to customize its contribution to autoscale (e.g., margins, excluded ranges). *Deferred — needs design.*

### Group 3: Line Style Variants ✅ `38beed1`
**Done:** Added `LargeDashed` (dash=6, gap=6) and `SparseDotted` (dash=1, gap=4) to `LineStyle` enum.
- ~~`Dotted`~~ (already existed)
- ~~`LargeDashed`~~
- ~~`SparseDotted`~~

### Group 4: LineType (Interpolation) ✅ `c50024b`
**Done:** `LineType` enum in `series.rs`, `line_type` field on `LineSeriesOptions`, `flush_line_segment()` helper in `chart_renderer.rs`.
- `Simple` (current — straight segments) ✅
- `WithSteps` — staircase interpolation (horizontal-then-vertical) ✅
- `Curved` — Catmull-Rom spline with 8 points per span ✅

### Group 5: LastPriceAnimationMode ✅ `18cee58`
**Done:** `LastPriceAnimationMode` enum in `series.rs`, `last_price_animation` field on `LineSeriesOptions`, `animation_frame` counter in `ChartState`.
- `Disabled` (default) ✅
- `Continuous` — always pulsing ✅
- `OnDataUpdate` — pulse when new data arrives ✅

### Group 6: CrosshairMode Magnet ✅ `721dcf0`
**Done:** `CrosshairMode` enum in `chart_options.rs`, `mode` field on `CrosshairOptions`. Snap logic in `pointer_move()`.
- `Normal` ✅ (current)
- `Magnet` — snap Y to bar close price ✅
- `Hidden` — suppress crosshair lines ✅

### Group 7: C-ABI Wrapper Refactor ✅ `092595d` + `c5e5720`
**Done:** All 8 functions moved to proper struct methods. See table above.

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
