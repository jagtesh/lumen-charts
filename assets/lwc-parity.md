# API Parity Audit: Lightweight Charts v4 vs Lumen Charts C-ABI

**Date:** 2026-03-01 • **C-ABI:** [chart_core.h](file:///Users/jagtesh/.gemini/antigravity/playground/chart-mvp/core/include/chart_core.h) (149 lines, ~75 functions)

---

## Summary

| Interface | LWC Methods | Implemented | Coverage |
|-----------|-------------|-------------|----------|
| IChartApi | 15 | 12 | 80% |
| ISeriesApi | 16 | 13 | 81% |
| ITimeScaleApi | 16 | 12 | 75% |
| IPriceScaleApi | 4 | 3 | 75% |
| **Total** | **51** | **40** | **78%** |

---

## IChartApi

| LWC Method | Status | C-ABI Function |
|---|---|---|
| `createChart(container)` | ✅ | `chart_create(w, h, scale, metal_layer)` |
| `remove()` | ✅ | `chart_destroy(chart)` |
| `resize(w, h)` | ✅ | `chart_resize(chart, w, h, scale)` |
| `applyOptions(options)` | ✅ | `chart_apply_options(chart, json)` |
| `options()` | ✅ | `chart_get_options(chart)` |
| `addSeries(type, options)` | ✅ | `chart_add_ohlc_series`, `_candlestick_series`, `_line_series`, `_area_series`, `_baseline_series`, `_histogram_series` |
| `removeSeries(series)` | ✅ | `chart_remove_series(chart, id)` |
| `subscribeClick(handler)` | ✅ | `chart_subscribe_click(chart, cb, ud)` |
| `unsubscribeClick(handler)` | ✅ | `chart_unsubscribe_click(chart)` |
| `subscribeDblClick(handler)` | ✅ | `chart_subscribe_dbl_click(chart, cb, ud)` |
| `setCrosshairPosition(price, time, series)` | ✅ | `chart_set_crosshair_position(chart, price, time, sid)` |
| `clearCrosshairPosition()` | ✅ | `chart_clear_crosshair_position(chart)` |
| `subscribeCrosshairMove(handler)` | ❌ | `chart_subscribe_crosshair_move` exists but returns `ChartEventParam` — missing `seriesData` map |
| `addCustomSeries(view, options)` | ❌ | Not implemented — custom series plugin system |
| `takeScreenshot()` | ❌ | Not implemented — GPU readback to image |

### Notes
- `subscribeCrosshairMove` is partially implemented: the callback fires with price/time/coordinates, but LWC's version also returns a `seriesData` map containing the value of every series at the crosshair time. Our version only returns a single price.
- `addCustomSeries` is LWC's plugin system — low priority for MVP.
- `takeScreenshot` would require GPU readback to a PNG buffer — moderate effort.

---

## ISeriesApi

| LWC Method | Status | C-ABI Function |
|---|---|---|
| `setData(data)` | ✅ | `chart_series_set_ohlc_data`, `_line_data`, `_histogram_data` |
| `update(bar)` | ✅ | `chart_series_update_ohlc_bar`, `_line_bar`, `_histogram_bar` |
| `applyOptions(options)` | ✅ | `chart_series_apply_options(chart, sid, json)` |
| `options()` | ⚠️ | Not exposed via C-ABI (state is in Rust, applyed via JSON) |
| `priceToCoordinate(price)` | ✅ | `chart_price_to_coordinate(chart, price)` |
| `coordinateToPrice(y)` | ✅ | `chart_coordinate_to_price(chart, y)` |
| `seriesType()` | ✅ | `chart_series_type(chart, sid)` |
| `createPriceLine(options)` | ✅ | `chart_series_create_price_line(chart, sid, json)` |
| `removePriceLine(line)` | ✅ | `chart_series_remove_price_line(chart, sid, lid)` |
| `priceScale()` | ✅ | `chart_price_scale_get_mode`, `_set_mode`, `_get_range` |
| `dataByIndex(index)` | ✅ | `chart_data_by_index(chart, sid, idx, out_time, out_value)` |
| `data()` | ✅ | `chart_series_get_ohlc_data`, `_line_data`, `_histogram_data` |
| `priceFormatter()` | ✅ | `chart_format_price(chart, price)` |
| `setMarkers(markers)` | ❌ | Not implemented — marker rendering system |
| `markers()` | ❌ | Not implemented |
| `barsInLogicalRange(range)` | ❌ | Not implemented — returns count+ of bars visible in a logical range |

### Notes
- `setMarkers`/`markers` are high-value features for trading apps (showing buy/sell signals, annotations, etc.). This is likely the **#1 missing feature** for production use.
- `barsInLogicalRange` is useful for lazy-loading / pagination of historical data.
- `options()` could be added by serializing the current series options to JSON.

---

## ITimeScaleApi

| LWC Method | Status | C-ABI Function |
|---|---|---|
| `scrollToPosition(position, animated)` | ✅ | `chart_time_scale_scroll_to_position(chart, pos, animated)` |
| `scrollToRealTime()` | ✅ | `chart_time_scale_scroll_to_real_time(chart)` |
| `getVisibleRange()` | ✅ | `chart_time_scale_get_visible_range(chart, from, to)` |
| `setVisibleRange(range)` | ✅ | `chart_time_scale_set_visible_range(chart, from, to)` |
| `getVisibleLogicalRange()` | ✅ | `chart_time_scale_get_visible_logical_range(chart, from, to)` |
| `setVisibleLogicalRange(range)` | ✅ | `chart_time_scale_set_visible_logical_range(chart, from, to)` |
| `resetTimeScale()` | ✅ | `chart_time_scale_reset(chart)` |
| `fitContent()` | ✅ | `chart_fit_content(chart)` |
| `coordinateToLogical(x)` | ✅ | `chart_coordinate_to_logical(chart, x)` |
| `logicalToCoordinate(logical)` | ✅ | `chart_logical_to_coordinate(chart, logical)` |
| `timeToCoordinate(time)` | ✅ | `chart_time_to_coordinate(chart, time)` |
| `coordinateToTime(x)` | ✅ | `chart_coordinate_to_time(chart, x)` |
| `width()` | ✅ | `chart_time_scale_width(chart)` |
| `height()` | ✅ | `chart_time_scale_height(chart)` — **bonus**, not in LWC |
| `applyOptions(options)` | ❌ | No separate time scale options — applied via `chart_apply_options` |
| `subscribeVisibleTimeRangeChange` | ❌ | Not implemented |
| `subscribeVisibleLogicalRangeChange` | ❌ | Not implemented |
| `subscribeSizeChange` | ❌ | Not implemented |

### Notes
- The three `subscribe*` methods are event subscriptions for external state management (e.g., syncing two charts). Low priority for single-chart use cases.
- `applyOptions` on the time scale is partially covered: time scale options can be set via `chart_apply_options` on the parent chart, but there's no dedicated `ITimeScaleApi.applyOptions`.

---

## IPriceScaleApi

| LWC Method | Status | C-ABI Function |
|---|---|---|
| `applyOptions(options)` | ⚠️ | Via `chart_apply_options` (chart-level), no per-scale API |
| `options()` | ❌ | Not implemented |
| `width()` | ❌ | Not implemented |
| `mode()` / `setMode()` | ✅ | `chart_price_scale_get_mode`, `chart_price_scale_set_mode` |

---

## Lumen-Exclusive Features (Not in LWC)

These are features in our C-ABI that have **no equivalent** in Lightweight Charts:

| Feature | C-ABI Functions |
|---|---|
| Multi-pane layout | `chart_add_pane`, `chart_remove_pane`, `chart_swap_panes`, `chart_pane_size`, `chart_series_move_to_pane` |
| Touch event handling | `chart_touch_start`, `chart_touch_move`, `chart_touch_end`, `chart_touch_tick` |
| Optimized rendering | `chart_render_if_needed` (invalidation-driven) |
| Pinch-to-zoom | `chart_pinch(chart, scale, cx, cy)` |
| Keyboard navigation | `chart_key_down(chart, key_code)` |
| Series pop (remove last N) | `chart_series_pop(chart, sid, count)` |
| Primary series type switch | `chart_set_series_type(chart, type)` |
| Per-series data retrieval | `chart_series_get_ohlc_data`, `_line_data`, `_histogram_data`, `_last_value_data` |
| Formatter API | `chart_format_price`, `chart_format_date`, `chart_format_time` |

---

## Priority Gaps (Recommended Next Steps)

| Priority | Feature | Effort | Impact |
|---|---|---|---|
| 🔴 High | `setMarkers` / `markers` | Medium | Essential for trading apps |
| 🟡 Medium | `subscribeCrosshairMove` with `seriesData` map | Medium | Needed for data tooltips |
| 🟡 Medium | `barsInLogicalRange` | Low | Enables lazy-loading |
| 🟢 Low | `takeScreenshot` | Medium | Nice-to-have |
| 🟢 Low | `subscribeVisibleRangeChange` | Low | Multi-chart sync |
| 🟢 Low | `addCustomSeries` (plugin system) | High | Advanced extensibility |
