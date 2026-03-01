// chart_core.h — C-ABI header for chart-core Rust library
#ifndef CHART_CORE_H
#define CHART_CORE_H

#include <stdint.h>
#include <stdbool.h>

typedef struct Chart Chart;

// Lifecycle
Chart* chart_create(uint32_t width, uint32_t height, double scale_factor, void* metal_layer);
void chart_render(Chart* chart);
void chart_resize(Chart* chart, uint32_t width, uint32_t height, double scale_factor);
void chart_destroy(Chart* chart);

// Interactions (all return bool: true if chart needs redraw)
bool chart_pointer_move(Chart* chart, float x, float y);
bool chart_pointer_down(Chart* chart, float x, float y, uint8_t button);
bool chart_pointer_up(Chart* chart, float x, float y, uint8_t button);
bool chart_pointer_leave(Chart* chart);
bool chart_scroll(Chart* chart, float delta_x, float delta_y);
bool chart_zoom(Chart* chart, float factor, float center_x);
bool chart_pinch(Chart* chart, float scale, float center_x, float center_y);
bool chart_fit_content(Chart* chart);
bool chart_key_down(Chart* chart, uint32_t key_code);
void chart_tick(Chart* chart);

// Data management
void chart_set_data(Chart* chart, const double* data, uint32_t count);
bool chart_update_bar(Chart* chart, int64_t time, double open, double high, double low, double close);
uint32_t chart_bar_count(Chart* chart);

// Series type (0=OHLC, 1=Candlestick, 2=Line)
bool chart_set_series_type(Chart* chart, uint32_t series_type);

// Multi-series management
uint32_t chart_add_ohlc_series(Chart* chart, const int64_t* times, const double* opens, const double* highs, const double* lows, const double* closes, uint32_t count);
uint32_t chart_add_candlestick_series(Chart* chart, const int64_t* times, const double* opens, const double* highs, const double* lows, const double* closes, uint32_t count);
uint32_t chart_add_line_series(Chart* chart, const int64_t* times, const double* values, uint32_t count);
uint32_t chart_add_area_series(Chart* chart, const int64_t* times, const double* values, uint32_t count);
uint32_t chart_add_baseline_series(Chart* chart, const int64_t* times, const double* values, uint32_t count, double base_value);
uint32_t chart_add_histogram_series(Chart* chart, const int64_t* times, const double* values, const uint32_t* colors, uint32_t count);
bool chart_remove_series(Chart* chart, uint32_t series_id);
uint32_t chart_series_count(const Chart* chart);

#endif // CHART_CORE_H
