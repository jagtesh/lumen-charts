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

// Events & Callbacks
typedef struct {
    int64_t time;
    double logical;
    float point_x;
    float point_y;
    double price;
} ChartEventParam;

typedef void (*ChartEventCallback)(const ChartEventParam* param, void* user_data);

void chart_subscribe_click(Chart* chart, ChartEventCallback callback, void* user_data);
void chart_unsubscribe_click(Chart* chart);

void chart_subscribe_crosshair_move(Chart* chart, ChartEventCallback callback, void* user_data);
void chart_unsubscribe_crosshair_move(Chart* chart);

bool chart_set_crosshair_position(Chart* chart, double price, int64_t time, uint32_t series_id);
bool chart_clear_crosshair_position(Chart* chart);

// Coordinate translation / Read APIs
float chart_price_to_coordinate(Chart* chart, double price);
double chart_coordinate_to_price(Chart* chart, float y);
float chart_logical_to_coordinate(Chart* chart, double logical);
double chart_coordinate_to_logical(Chart* chart, float x);
float chart_time_to_coordinate(Chart* chart, int64_t time);
int64_t chart_coordinate_to_time(Chart* chart, float x);

// Data Retrieval
uint32_t chart_series_data_length(Chart* chart, uint32_t series_id);
uint32_t chart_series_get_ohlc_data(Chart* chart, uint32_t series_id, int64_t* times, double* opens, double* highs, double* lows, double* closes, uint32_t max_count);
uint32_t chart_series_get_line_data(Chart* chart, uint32_t series_id, int64_t* times, double* values, uint32_t max_count);
uint32_t chart_series_get_histogram_data(Chart* chart, uint32_t series_id, int64_t* times, double* values, uint32_t max_count);
bool chart_series_get_last_value_data(Chart* chart, uint32_t series_id, int64_t* out_time, double* out_value);

// Data management
void chart_set_data(Chart* chart, const double* data, uint32_t count);
bool chart_update_bar(Chart* chart, int64_t time, double open, double high, double low, double close);
uint32_t chart_bar_count(Chart* chart);

// Series-specific Data management
bool chart_series_set_ohlc_data(Chart* chart, uint32_t series_id, const int64_t* times, const double* opens, const double* highs, const double* lows, const double* closes, uint32_t count);
bool chart_series_set_line_data(Chart* chart, uint32_t series_id, const int64_t* times, const double* values, uint32_t count);
bool chart_series_set_histogram_data(Chart* chart, uint32_t series_id, const int64_t* times, const double* values, const uint32_t* colors, uint32_t count);
bool chart_series_update_ohlc_bar(Chart* chart, uint32_t series_id, int64_t time, double open, double high, double low, double close);
bool chart_series_update_line_bar(Chart* chart, uint32_t series_id, int64_t time, double value);
bool chart_series_update_histogram_bar(Chart* chart, uint32_t series_id, int64_t time, double value, uint32_t color_rgba, bool has_color);
bool chart_series_pop(Chart* chart, uint32_t series_id, uint32_t count);

// Price Lines
uint32_t chart_series_create_price_line(Chart* chart, uint32_t series_id, const char* options_json);
bool chart_series_remove_price_line(Chart* chart, uint32_t series_id, uint32_t line_id);

// Series type (0=OHLC, 1=Candlestick, 2=Line)
bool chart_set_series_type(Chart* chart, uint32_t series_type);

// Options Management
bool chart_apply_options(Chart* chart, const char* json_cstr);
bool chart_series_apply_options(Chart* chart, uint32_t series_id, const char* json_cstr);

// Multi-series management
uint32_t chart_add_ohlc_series(Chart* chart, const int64_t* times, const double* opens, const double* highs, const double* lows, const double* closes, uint32_t count);
uint32_t chart_add_candlestick_series(Chart* chart, const int64_t* times, const double* opens, const double* highs, const double* lows, const double* closes, uint32_t count);
uint32_t chart_add_line_series(Chart* chart, const int64_t* times, const double* values, uint32_t count);
uint32_t chart_add_area_series(Chart* chart, const int64_t* times, const double* values, uint32_t count);
uint32_t chart_add_baseline_series(Chart* chart, const int64_t* times, const double* values, uint32_t count, double base_value);
uint32_t chart_add_histogram_series(Chart* chart, const int64_t* times, const double* values, const uint32_t* colors, uint32_t count);
bool chart_remove_series(Chart* chart, uint32_t series_id);
uint32_t chart_series_count(const Chart* chart);

// Multi-pane management
uint32_t chart_add_pane(Chart* chart, float height_stretch);
bool chart_remove_pane(Chart* chart, uint32_t pane_id);
bool chart_series_move_to_pane(Chart* chart, uint32_t series_id, uint32_t pane_id);
uint32_t chart_pane_count(const Chart* chart);
bool chart_swap_panes(Chart* chart, uint32_t pane_id_a, uint32_t pane_id_b);
bool chart_pane_size(const Chart* chart, uint32_t pane_id, float* out_x, float* out_y, float* out_width, float* out_height);

// Double-click events
void chart_subscribe_dbl_click(Chart* chart, ChartEventCallback callback, void* user_data);
void chart_unsubscribe_dbl_click(Chart* chart);

// Optimized rendering
bool chart_render_if_needed(Chart* chart);

// Touch events
bool chart_touch_start(Chart* chart, uint32_t id, float x, float y);
bool chart_touch_move(Chart* chart, uint32_t id, float x, float y);
bool chart_touch_end(Chart* chart, uint32_t id);
void chart_touch_tick(Chart* chart);

// IChartApi
const char* chart_get_options(Chart* chart);
void chart_free_string(const char* ptr);

// ITimeScaleApi
void chart_time_scale_scroll_to_position(Chart* chart, float position, bool animated);
void chart_time_scale_scroll_to_real_time(Chart* chart);
bool chart_time_scale_get_visible_range(Chart* chart, int64_t* out_from, int64_t* out_to);
void chart_time_scale_set_visible_range(Chart* chart, int64_t from, int64_t to);
bool chart_time_scale_get_visible_logical_range(Chart* chart, float* out_from, float* out_to);
void chart_time_scale_set_visible_logical_range(Chart* chart, float from, float to);
void chart_time_scale_reset(Chart* chart);
float chart_time_scale_width(const Chart* chart);
float chart_time_scale_height(const Chart* chart);

// ISeriesApi
uint32_t chart_series_type(const Chart* chart, uint32_t series_id);
bool chart_data_by_index(const Chart* chart, uint32_t series_id, int32_t index, int64_t* out_time, double* out_value);

// IPriceScaleApi
uint32_t chart_price_scale_get_mode(const Chart* chart);
void chart_price_scale_set_mode(Chart* chart, uint32_t mode);
bool chart_price_scale_get_range(const Chart* chart, double* out_min, double* out_max);

// Localization / Formatters
const char* chart_format_price(Chart* chart, double price);
const char* chart_format_date(Chart* chart, int64_t timestamp);
const char* chart_format_time(Chart* chart, int64_t timestamp);

#endif // CHART_CORE_H
