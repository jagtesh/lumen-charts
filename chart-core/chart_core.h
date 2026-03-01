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

#endif // CHART_CORE_H
