# API Parity Audit: Lightweight Charts v4 vs Lumen Charts C-ABI

**Date:** 2026-03-01 • **C-ABI:** [chart_core.h](../core/include/chart_core.h) (~90 functions)

---

## Summary

| Interface | LWC Methods | Implemented | Coverage |
|-----------|-------------|-------------|----------|
| IChartApi | 15 | 12 | 80% |
| ISeriesApi | 16 | 16 | **100%** |
| ITimeScaleApi | 16 | 16 | **100%** |
| IPriceScaleApi | 4 | 4 | **100%** |
| **Total** | **51** | **48** | **94%** |

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
| `setCrosshairPosition(…)` | ✅ | `chart_set_crosshair_position(chart, price, time, sid)` |
| `clearCrosshairPosition()` | ✅ | `chart_clear_crosshair_position(chart)` |
| `subscribeCrosshairMove(handler)` | ✅ | `chart_subscribe_crosshair_move` + `chart_crosshair_get_series_data` |
| `addCustomSeries(view, options)` | ❌ | Plugin system — not planned for v1 |
| `takeScreenshot()` | ❌ | GPU readback — deferred |

---

## ISeriesApi — 100% ✅

| LWC Method | C-ABI Function |
|---|---|
| `setData(data)` | `chart_series_set_ohlc_data`, `_line_data`, `_histogram_data` |
| `update(bar)` | `chart_series_update_ohlc_bar`, `_line_bar`, `_histogram_bar` |
| `applyOptions(options)` | `chart_series_apply_options(chart, sid, json)` |
| `options()` | `chart_series_get_options(chart, sid) → json` |
| `priceToCoordinate(price)` | `chart_price_to_coordinate(chart, price)` |
| `coordinateToPrice(y)` | `chart_coordinate_to_price(chart, y)` |
| `seriesType()` | `chart_series_type(chart, sid)` |
| `createPriceLine(options)` | `chart_series_create_price_line(chart, sid, json)` |
| `removePriceLine(line)` | `chart_series_remove_price_line(chart, sid, lid)` |
| `priceScale()` | `chart_price_scale_get_mode`, `_set_mode`, `_get_range` |
| `dataByIndex(index)` | `chart_data_by_index(chart, sid, idx, out_time, out_value)` |
| `data()` | `chart_series_get_ohlc_data`, `_line_data`, `_histogram_data` |
| `priceFormatter()` | `chart_format_price(chart, price)` |
| `setMarkers(markers)` | `chart_series_set_markers(chart, sid, json)` |
| `markers()` | `chart_series_markers(chart, sid) → json` |
| `barsInLogicalRange(range)` | `chart_series_bars_in_logical_range(chart, sid, from, to)` |

---

## ITimeScaleApi — 100% ✅

| LWC Method | C-ABI Function |
|---|---|
| `scrollToPosition(pos, animated)` | `chart_time_scale_scroll_to_position` |
| `scrollToRealTime()` | `chart_time_scale_scroll_to_real_time` |
| `getVisibleRange()` | `chart_time_scale_get_visible_range` |
| `setVisibleRange(range)` | `chart_time_scale_set_visible_range` |
| `getVisibleLogicalRange()` | `chart_time_scale_get_visible_logical_range` |
| `setVisibleLogicalRange(range)` | `chart_time_scale_set_visible_logical_range` |
| `resetTimeScale()` | `chart_time_scale_reset` |
| `fitContent()` | `chart_fit_content` |
| `coordinateToLogical(x)` | `chart_coordinate_to_logical` |
| `logicalToCoordinate(logical)` | `chart_logical_to_coordinate` |
| `timeToCoordinate(time)` | `chart_time_to_coordinate` |
| `coordinateToTime(x)` | `chart_coordinate_to_time` |
| `width()` / `height()` | `chart_time_scale_width`, `chart_time_scale_height` |
| `applyOptions(options)` | `chart_time_scale_apply_options(chart, json)` |
| `subscribeVisibleTimeRangeChange` | `chart_time_scale_subscribe_visible_time_range_change` |
| `subscribeVisibleLogicalRangeChange` | `chart_time_scale_subscribe_visible_logical_range_change` |
| `subscribeSizeChange` | `chart_time_scale_subscribe_size_change` |

---

## IPriceScaleApi — 100% ✅

| LWC Method | C-ABI Function |
|---|---|
| `mode()` / `setMode()` | `chart_price_scale_get_mode`, `chart_price_scale_set_mode` |
| `applyOptions(options)` | `chart_price_scale_apply_options(chart, json)` |
| `width()` | `chart_price_scale_width(chart)` |

---

## Lumen-Exclusive Features (Not in LWC)

| Feature | C-ABI Functions |
|---|---|
| Multi-pane layout | `chart_add_pane`, `chart_remove_pane`, `chart_swap_panes`, `chart_pane_size`, `chart_series_move_to_pane` |
| Touch events | `chart_touch_start`, `chart_touch_move`, `chart_touch_end`, `chart_touch_tick` |
| Optimized rendering | `chart_render_if_needed` (invalidation-driven) |
| Pinch-to-zoom | `chart_pinch(chart, scale, cx, cy)` |
| Keyboard navigation | `chart_key_down(chart, key_code)` |
| Series pop (remove last N) | `chart_series_pop(chart, sid, count)` |
| Primary series type switch | `chart_set_series_type(chart, type)` |
| Formatters | `chart_format_price`, `chart_format_date`, `chart_format_time` |

---

## Remaining Gaps (3 methods, deferred)

| Feature | Effort | Reason |
|---|---|---|
| `addCustomSeries` | High | Plugin system — advanced extensibility |
| `takeScreenshot` | High | GPU readback + PNG encoding |
| `IPriceScaleApi.options()` | Low | Serialize price scale options to JSON |
