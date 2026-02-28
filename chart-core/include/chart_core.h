#ifndef CHART_CORE_H
#define CHART_CORE_H

#include <stdint.h>

/// Opaque chart handle
typedef struct Chart Chart;

/// Create a new chart from sample data.
/// `metal_layer` must be a pointer to a CAMetalLayer.
/// Returns an opaque handle — caller must eventually call chart_destroy().
Chart* chart_create(uint32_t width, uint32_t height, double scale_factor, void* metal_layer);

/// Render the chart to the surface provided at creation.
void chart_render(Chart* chart);

/// Resize the chart. Call when the view/window size changes.
void chart_resize(Chart* chart, uint32_t width, uint32_t height, double scale_factor);

/// Destroy the chart and free all GPU resources.
void chart_destroy(Chart* chart);

#endif /* CHART_CORE_H */
