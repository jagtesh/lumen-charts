#![allow(unused_unsafe)]
//! Lumen Charts SDK — Safe, idiomatic Rust API (v5 compatible)
//!
//! This crate wraps the `lumen-charts` core with a high-level API that mirrors
//! LWC v5's `IChartApi`, `ISeriesApi`, `IPaneApi`, `ITimeScaleApi`, and
//! `IPriceScaleApi` interfaces.
//!
//! # Design Notes
//!
//! Unlike the Swift and JS SDKs (which go through the C-ABI), this SDK accesses
//! `Chart.state` directly — Rust-to-Rust, no FFI overhead, full type safety.
//! The C-ABI functions in `lumen_charts_core::lib` exist for foreign consumers; Rust
//! consumers should use this SDK instead.
//!
//! # Example
//!
//! ```ignore
//! use lumen_charts_sdk::{ChartApi, SeriesDefinition, OhlcBar};
//!
//! let mut chart = ChartApi::new(chart);
//! let series = chart.add_series(SeriesDefinition::Candlestick);
//! series.set_ohlc_data(&bars);
//! let pane = chart.add_pane(0.3);
//! series.move_to_pane(&pane);
//! ```

// Re-export core types consumers will need
pub use lumen_charts_core::chart_model::OhlcBar;
pub use lumen_charts_core::color::{Color, ColorName};
pub use lumen_charts_core::renderers::Renderer;
pub use lumen_charts_core::series::{
    AreaSeriesOptions, BaselineSeriesOptions, CandlestickOptions, HistogramDataPoint,
    HistogramSeriesOptions, LineDataPoint, LineSeriesOptions, PriceLineOptions, SeriesType,
};
pub use lumen_charts_core::Chart;

use lumen_charts_core::series::Series;
use std::ffi::CString;

// ---------------------------------------------------------------------------
// v5: SeriesDefinition (unified addSeries entry point)
// ---------------------------------------------------------------------------

/// Defines the type of series to add.
///
/// v5 alignment: replaces per-type `addOhlcSeries`, `addCandlestickSeries`, etc.
/// with a single `chart.add_series(definition)` entry point.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum SeriesDefinition {
    Ohlc,
    Candlestick,
    Line,
    Area,
    Histogram,
    Baseline { base_value: f64 },
}

// ---------------------------------------------------------------------------
// v5: MouseEventParams
// ---------------------------------------------------------------------------

/// Event parameters from click/crosshair/dbl-click callbacks (v5 model).
///
/// Mirrors LWC v5 `MouseEventParams` with pane awareness, hovered series,
/// and pull-based series data.
#[derive(Debug, Clone)]
pub struct MouseEventParams {
    pub time: i64,
    pub logical: f64,
    pub point: (f32, f32),
    pub price: f64,
    /// Index of the pane where the event occurred
    pub pane_index: u32,
    /// ID of the series under the cursor (0 if none)
    pub hovered_series_id: u32,
    /// Series data at this crosshair position: Vec<(series_id, value)>
    pub series_data: Vec<(u32, f64)>,
}

// ---------------------------------------------------------------------------
// v5: PaneApi
// ---------------------------------------------------------------------------

/// Handle to a chart pane (v5: index-based identity).
///
/// Pane indices shift when panes are removed. Pane 0 is the main (always exists).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PaneApi {
    index: u32,
}

impl PaneApi {
    /// Get the pane index.
    pub fn pane_index(&self) -> u32 {
        self.index
    }
}

// ---------------------------------------------------------------------------
// v5: SeriesApi
// ---------------------------------------------------------------------------

/// Handle to a chart series. Provides v5 `ISeriesApi` methods.
///
/// All methods take `&mut ChartApi` to access the chart state. This is a
/// lightweight handle (just the series ID) — the actual data lives in ChartApi.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SeriesApi {
    id: u32,
}

impl SeriesApi {
    /// Get the series ID.
    pub fn id(&self) -> u32 {
        self.id
    }

    /// Get the series type.
    pub fn series_type(&self, chart: &ChartApi) -> Option<SeriesType> {
        chart.inner.state.series.get(self.id).map(|s| s.series_type)
    }

    // -- Data management --

    /// Set OHLC data for this series.
    pub fn set_ohlc_data(&self, chart: &mut ChartApi, data: &[OhlcBar]) {
        if let Some(series) = chart.inner.state.series.get_mut(self.id) {
            series.data.set_ohlc(data.to_vec());
            chart.invalidate();
        }
    }

    /// Set line/area/baseline data for this series.
    pub fn set_line_data(&self, chart: &mut ChartApi, data: &[LineDataPoint]) {
        if let Some(series) = chart.inner.state.series.get_mut(self.id) {
            series.data.set_line(data.to_vec());
            chart.invalidate();
        }
    }

    /// Set histogram data for this series.
    pub fn set_histogram_data(&self, chart: &mut ChartApi, data: &[HistogramDataPoint]) {
        if let Some(series) = chart.inner.state.series.get_mut(self.id) {
            series.data.set_histogram(data.to_vec());
            chart.invalidate();
        }
    }

    /// Update (or append) a single OHLC bar.
    pub fn update_ohlc(&self, chart: &mut ChartApi, bar: OhlcBar) {
        if let Some(series) = chart.inner.state.series.get_mut(self.id) {
            series.data.update_ohlc(bar);
            chart.invalidate();
        }
    }

    /// Update (or append) a single line/area/baseline point.
    pub fn update_line(&self, chart: &mut ChartApi, pt: LineDataPoint) {
        if let Some(series) = chart.inner.state.series.get_mut(self.id) {
            series.data.update_line(pt);
            chart.invalidate();
        }
    }

    /// Update (or append) a single histogram point.
    pub fn update_histogram(&self, chart: &mut ChartApi, pt: HistogramDataPoint) {
        if let Some(series) = chart.inner.state.series.get_mut(self.id) {
            series.data.update_histogram(pt);
            chart.invalidate();
        }
    }

    /// Number of data points.
    pub fn data_length(&self, chart: &ChartApi) -> usize {
        chart
            .inner
            .state
            .series
            .get(self.id)
            .map(|s| s.data.len())
            .unwrap_or(0)
    }

    /// Remove `count` items from the end of the series.
    pub fn pop(&self, chart: &mut ChartApi, count: usize) {
        if let Some(series) = chart.inner.state.series.get_mut(self.id) {
            series.data.pop(count);
            chart.invalidate();
        }
    }

    // -- Options --

    /// Apply a partial JSON options string (e.g. `{"color":[1,0,0,1]}`).
    pub fn apply_options(&self, chart: &mut ChartApi, json: &str) -> bool {
        let ok = chart
            .inner
            .state
            .series
            .get_mut(self.id)
            .map(|s| s.apply_options_json(json))
            .unwrap_or(false);
        if ok {
            chart.invalidate();
        }
        ok
    }

    /// Get the current series options as a JSON string.
    pub fn options(&self, chart: &ChartApi) -> Option<String> {
        let ptr = unsafe {
            lumen_charts_core::chart_series_get_options(&chart.inner as *const Chart, self.id)
        };
        if ptr.is_null() {
            return None;
        }
        let s = unsafe { std::ffi::CStr::from_ptr(ptr).to_string_lossy().into_owned() };
        unsafe { lumen_charts_core::chart_free_string(ptr) };
        Some(s)
    }

    // -- Visibility --

    /// Set series visibility.
    pub fn set_visible(&self, chart: &mut ChartApi, visible: bool) {
        if let Some(series) = chart.inner.state.series.get_mut(self.id) {
            series.visible = visible;
            chart.invalidate();
        }
    }

    /// Get series visibility.
    pub fn visible(&self, chart: &ChartApi) -> bool {
        chart
            .inner
            .state
            .series
            .get(self.id)
            .map(|s| s.visible)
            .unwrap_or(false)
    }

    // -- Pane (v5) --

    /// Get the pane this series belongs to (v5: `ISeriesApi.getPane()`).
    pub fn get_pane(&self, chart: &ChartApi) -> Option<PaneApi> {
        chart.inner.state.series.get(self.id).map(|s| PaneApi {
            index: s.pane_index as u32,
        })
    }

    /// Move this series to a different pane.
    pub fn move_to_pane(&self, chart: &mut ChartApi, pane: &PaneApi) -> bool {
        let ok = chart.inner.state.move_series_to_pane(self.id, pane.index);
        if ok {
            chart.invalidate();
        }
        ok
    }

    /// Get the z-order of this series within its pane (v5: `seriesOrder()`).
    pub fn series_order(&self, chart: &ChartApi) -> Option<u32> {
        let series = chart.inner.state.series.get(self.id)?;
        let pane_idx = series.pane_index;
        let mut order = 0u32;
        for s in chart.inner.state.series.series.iter() {
            if s.pane_index == pane_idx {
                if s.id == self.id {
                    return Some(order);
                }
                order += 1;
            }
        }
        None
    }

    /// Set the z-order of this series within its pane (v5: `setSeriesOrder(order)`).
    pub fn set_series_order(&self, chart: &mut ChartApi, order: u32) -> bool {
        // Delegate to the C-ABI function which handles the reordering logic
        let ok = unsafe {
            lumen_charts_core::chart_series_set_order(&mut chart.inner as *mut Chart, self.id, order)
        };
        if ok {
            chart.invalidate();
        }
        ok
    }

    // -- Price Lines --

    /// Create a price line on this series. Returns the price line ID.
    pub fn create_price_line(&self, chart: &mut ChartApi, options: PriceLineOptions) -> u32 {
        if let Some(series) = chart.inner.state.series.get_mut(self.id) {
            let id = series.add_price_line(options);
            chart.invalidate();
            id
        } else {
            u32::MAX
        }
    }

    /// Remove a price line by ID.
    pub fn remove_price_line(&self, chart: &mut ChartApi, line_id: u32) -> bool {
        if let Some(series) = chart.inner.state.series.get_mut(self.id) {
            let ok = series.remove_price_line(line_id);
            if ok {
                chart.invalidate();
            }
            ok
        } else {
            false
        }
    }

    // -- Markers --

    /// Set markers on this series from a JSON array.
    pub fn set_markers(&self, chart: &mut ChartApi, markers_json: &str) -> bool {
        if let Ok(c_str) = CString::new(markers_json) {
            let ok = unsafe {
                lumen_charts_core::chart_series_set_markers(
                    &mut chart.inner as *mut Chart,
                    self.id,
                    c_str.as_ptr(),
                )
            };
            if ok {
                chart.invalidate();
            }
            ok
        } else {
            false
        }
    }

    /// Get markers as a JSON string.
    pub fn markers(&self, chart: &ChartApi) -> Option<String> {
        let ptr =
            unsafe { lumen_charts_core::chart_series_markers(&chart.inner as *const Chart, self.id) };
        if ptr.is_null() {
            return None;
        }
        let s = unsafe { std::ffi::CStr::from_ptr(ptr).to_string_lossy().into_owned() };
        unsafe { lumen_charts_core::chart_free_string(ptr) };
        Some(s)
    }

    /// Number of bars in a logical index range.
    pub fn bars_in_logical_range(&self, chart: &ChartApi, from: f32, to: f32) -> u32 {
        unsafe {
            lumen_charts_core::chart_series_bars_in_logical_range(
                &chart.inner as *const Chart,
                self.id,
                from,
                to,
            )
        }
    }
}

// ---------------------------------------------------------------------------
// v5: TimeScaleApi
// ---------------------------------------------------------------------------

/// Provides v5 `ITimeScaleApi` methods. Borrows `ChartApi`.
pub struct TimeScaleApi<'a> {
    chart: &'a mut ChartApi,
}

impl<'a> TimeScaleApi<'a> {
    /// Scroll to a specific bar position (fractional index from the right).
    pub fn scroll_to_position(&mut self, position: f32) {
        unsafe {
            lumen_charts_core::chart_time_scale_scroll_to_position(
                &mut self.chart.inner as *mut Chart,
                position,
            );
        }
        self.chart.invalidate();
    }

    /// Scroll so the last bar is visible (right edge).
    pub fn scroll_to_real_time(&mut self) {
        unsafe {
            lumen_charts_core::chart_time_scale_scroll_to_real_time(&mut self.chart.inner as *mut Chart);
        }
        self.chart.invalidate();
    }

    /// Get the visible time range as unix timestamps.
    pub fn get_visible_range(&self) -> Option<(i64, i64)> {
        let mut start = 0i64;
        let mut end = 0i64;
        let ok = unsafe {
            lumen_charts_core::chart_time_scale_get_visible_range(
                &self.chart.inner as *const Chart,
                &mut start,
                &mut end,
            )
        };
        if ok {
            Some((start, end))
        } else {
            None
        }
    }

    /// Set the visible time range by start/end timestamps.
    pub fn set_visible_range(&mut self, start: i64, end: i64) {
        unsafe {
            lumen_charts_core::chart_time_scale_set_visible_range(
                &mut self.chart.inner as *mut Chart,
                start,
                end,
            );
        }
        self.chart.invalidate();
    }

    /// Get the visible logical range (bar indices).
    pub fn get_visible_logical_range(&self) -> Option<(f64, f64)> {
        let mut first = 0f64;
        let mut last = 0f64;
        let ok = unsafe {
            lumen_charts_core::chart_time_scale_get_visible_logical_range(
                &self.chart.inner as *const Chart,
                &mut first,
                &mut last,
            )
        };
        if ok {
            Some((first, last))
        } else {
            None
        }
    }

    /// Set the visible logical range by bar indices.
    pub fn set_visible_logical_range(&mut self, first: f64, last: f64) {
        unsafe {
            lumen_charts_core::chart_time_scale_set_visible_logical_range(
                &mut self.chart.inner as *mut Chart,
                first,
                last,
            );
        }
        self.chart.invalidate();
    }

    /// Reset time scale to default (fit content).
    pub fn reset(&mut self) {
        unsafe {
            lumen_charts_core::chart_time_scale_reset(&mut self.chart.inner as *mut Chart);
        }
        self.chart.invalidate();
    }

    /// Get the time scale width in logical pixels.
    pub fn width(&self) -> f32 {
        unsafe { lumen_charts_core::chart_time_scale_width(&self.chart.inner as *const Chart) }
    }

    /// Get the time scale height in logical pixels.
    pub fn height(&self) -> f32 {
        unsafe { lumen_charts_core::chart_time_scale_height(&self.chart.inner as *const Chart) }
    }

    /// Apply options via JSON.
    pub fn apply_options(&mut self, json: &str) -> bool {
        if let Ok(c_str) = CString::new(json) {
            let ok = unsafe {
                lumen_charts_core::chart_time_scale_apply_options(
                    &mut self.chart.inner as *mut Chart,
                    c_str.as_ptr(),
                )
            };
            if ok {
                self.chart.invalidate();
            }
            ok
        } else {
            false
        }
    }
}

// ---------------------------------------------------------------------------
// v5: PriceScaleApi
// ---------------------------------------------------------------------------

/// Provides v5 `IPriceScaleApi` methods. Borrows `ChartApi`.
pub struct PriceScaleApi<'a> {
    chart: &'a mut ChartApi,
    pane_index: u32,
}

impl<'a> PriceScaleApi<'a> {
    /// Get the price scale mode: 0 = Normal, 1 = Logarithmic.
    pub fn mode(&self) -> u8 {
        unsafe {
            lumen_charts_core::chart_price_scale_get_mode(
                &self.chart.inner as *const Chart,
                self.pane_index,
            )
        }
    }

    /// Set the price scale mode: 0 = Normal, 1 = Logarithmic.
    pub fn set_mode(&mut self, mode: u8) {
        unsafe {
            lumen_charts_core::chart_price_scale_set_mode(
                &mut self.chart.inner as *mut Chart,
                self.pane_index,
                mode,
            );
        }
        self.chart.invalidate();
    }

    /// Get the current visible price range.
    pub fn range(&self) -> Option<(f64, f64)> {
        let mut min = 0f64;
        let mut max = 0f64;
        let ok = unsafe {
            lumen_charts_core::chart_price_scale_get_range(
                &self.chart.inner as *const Chart,
                self.pane_index,
                &mut min,
                &mut max,
            )
        };
        if ok {
            Some((min, max))
        } else {
            None
        }
    }

    /// Get the price scale width in pixels.
    pub fn width(&self) -> f32 {
        unsafe { lumen_charts_core::chart_price_scale_width(&self.chart.inner as *const Chart, 0) }
    }

    /// Apply options via JSON.
    pub fn apply_options(&mut self, json: &str) -> bool {
        if let Ok(c_str) = CString::new(json) {
            let ok = unsafe {
                lumen_charts_core::chart_price_scale_apply_options(
                    &mut self.chart.inner as *mut Chart,
                    c_str.as_ptr(),
                )
            };
            if ok {
                self.chart.invalidate();
            }
            ok
        } else {
            false
        }
    }
}

// ---------------------------------------------------------------------------
// v5: ChartApi — the main entry point
// ---------------------------------------------------------------------------

/// Safe, idiomatic wrapper around `lumen_charts_core::Chart`.
///
/// Provides the v5 `IChartApi` interface: unified `add_series`, pane management,
/// event subscriptions, coordinate translation, and sub-API accessors.
///
/// # Ownership
///
/// `ChartApi` takes ownership of the `Chart`. Drop it to release all resources.
pub struct ChartApi {
    pub inner: Chart,
}

impl ChartApi {
    /// Wrap an existing `Chart` in the v5 SDK.
    pub fn new(chart: Chart) -> Self {
        Self { inner: chart }
    }

    /// Create a chart with a custom renderer.
    pub fn with_renderer(
        renderer: Box<dyn Renderer>,
        width: u32,
        height: u32,
        scale_factor: f64,
    ) -> Self {
        Self {
            inner: Chart::new_with_renderer(renderer, width, height, scale_factor),
        }
    }

    // -- Rendering --

    /// Render the chart unconditionally.
    pub fn render(&mut self) {
        self.inner.render();
    }

    /// Render only if the invalidation mask says a redraw is needed.
    pub fn render_if_needed(&mut self) -> bool {
        self.inner.render_if_needed()
    }

    /// Mark the chart as needing a redraw (used internally after state mutations).
    fn invalidate(&mut self) {
        self.inner
            .state
            .pending_mask
            .set_global(lumen_charts_core::invalidation::InvalidationLevel::Full);
    }

    // -- Viewport --

    /// Resize the chart viewport.
    pub fn resize(&mut self, width: u32, height: u32, scale_factor: f64) {
        self.inner.resize(width, height, scale_factor);
    }

    /// Fit all data into the visible viewport.
    pub fn fit_content(&mut self) {
        self.inner.fit_content();
    }

    // -- Input --

    /// Handle a pointer/mouse move. Returns true if a redraw is needed.
    pub fn pointer_move(&mut self, x: f32, y: f32) -> bool {
        self.inner.pointer_move(x, y)
    }

    /// Handle a pointer/mouse button press. Returns true if a redraw is needed.
    pub fn pointer_down(&mut self, x: f32, y: f32, button: u8) -> bool {
        self.inner.pointer_down(x, y, button)
    }

    /// Handle a pointer/mouse button release. Returns true if a redraw is needed.
    pub fn pointer_up(&mut self, x: f32, y: f32, button: u8) -> bool {
        self.inner.pointer_up(x, y, button)
    }

    /// Handle pointer leaving the chart area. Returns true if a redraw is needed.
    pub fn pointer_leave(&mut self) -> bool {
        self.inner.pointer_leave()
    }

    /// Handle a scroll/wheel event. Returns true if a redraw is needed.
    pub fn scroll(&mut self, dx: f32, dy: f32) -> bool {
        self.inner.scroll(dx, dy)
    }

    /// Handle a zoom event (e.g. scroll-wheel zoom). Returns true if a redraw is needed.
    pub fn zoom(&mut self, factor: f32, center_x: f32) -> bool {
        unsafe { lumen_charts_core::chart_zoom(&mut self.inner as *mut Chart, factor, center_x) }
    }

    /// Handle a pinch-to-zoom gesture. Returns true if a redraw is needed.
    pub fn pinch(&mut self, scale: f32, center_x: f32, center_y: f32) -> bool {
        unsafe {
            lumen_charts_core::chart_pinch(&mut self.inner as *mut Chart, scale, center_x, center_y)
        }
    }

    /// Handle a keyboard key-down event. Returns true if a redraw is needed.
    pub fn key_down(&mut self, key_code: u32) -> bool {
        self.inner.key_down(key_code)
    }

    // -- Data (primary series, for backwards compat) --

    /// Set primary OHLC data from a slice of bars.
    pub fn set_data(&mut self, bars: Vec<OhlcBar>) {
        self.inner.set_data(bars);
    }

    /// Set primary series rendering type.
    pub fn set_series_type(&mut self, type_index: u32) {
        self.inner.set_series_type(type_index);
    }

    // -- v5: Unified addSeries --

    /// Add a new series to the chart (v5 unified API).
    ///
    /// Returns a `SeriesApi` handle for further manipulation.
    pub fn add_series(&mut self, definition: SeriesDefinition) -> SeriesApi {
        let series = match definition {
            SeriesDefinition::Ohlc => Series::ohlc(0, vec![]),
            SeriesDefinition::Candlestick => Series::candlestick(0, vec![]),
            SeriesDefinition::Line => Series::line(0, vec![]),
            SeriesDefinition::Area => Series::area(0, vec![]),
            SeriesDefinition::Histogram => Series::histogram(0, vec![]),
            SeriesDefinition::Baseline { base_value } => Series::baseline(0, vec![], base_value),
        };
        let id = self.inner.state.series.add(series);
        self.invalidate();
        SeriesApi { id }
    }

    /// Remove a series from the chart.
    pub fn remove_series(&mut self, series: &SeriesApi) -> bool {
        let ok = self.inner.state.series.remove(series.id);
        if ok {
            self.invalidate();
        }
        ok
    }

    /// Get the number of series.
    pub fn series_count(&self) -> usize {
        self.inner.state.series.len()
    }

    // -- v5: Pane management --

    /// Add a new pane. Returns a `PaneApi` handle. `height_stretch` controls
    /// relative height (1.0 = equal share).
    pub fn add_pane(&mut self, height_stretch: f32) -> PaneApi {
        let index = self.inner.state.add_pane(height_stretch);
        PaneApi { index }
    }

    /// Remove a pane by handle. Pane 0 (main) cannot be removed.
    /// Orphaned series move to pane 0.
    pub fn remove_pane(&mut self, pane: &PaneApi) -> bool {
        self.inner.state.remove_pane(pane.index)
    }

    /// Swap two panes.
    pub fn swap_panes(&mut self, a: &PaneApi, b: &PaneApi) -> bool {
        self.inner.state.swap_panes(a.index, b.index)
    }

    /// Get the number of panes.
    pub fn pane_count(&self) -> usize {
        self.inner.state.panes.len()
    }

    /// Get the layout rect of a pane: (x, y, width, height).
    pub fn pane_size(&self, pane: &PaneApi) -> Option<(f32, f32, f32, f32)> {
        self.inner.state.pane_size(pane.index)
    }

    // -- Options --

    /// Apply chart options from a JSON string.
    pub fn apply_options(&mut self, json: &str) -> bool {
        if let Ok(c_str) = CString::new(json) {
            let ok = unsafe {
                lumen_charts_core::chart_apply_options(&mut self.inner as *mut Chart, c_str.as_ptr())
            };
            if ok {
                self.invalidate();
            }
            ok
        } else {
            false
        }
    }

    /// Get current chart options as a JSON string.
    pub fn options(&self) -> Option<String> {
        let ptr = unsafe { lumen_charts_core::chart_get_options(&self.inner as *const Chart) };
        if ptr.is_null() {
            return None;
        }
        let s = unsafe { std::ffi::CStr::from_ptr(ptr).to_string_lossy().into_owned() };
        unsafe { lumen_charts_core::chart_free_string(ptr) };
        Some(s)
    }

    // -- Coordinate translation --

    /// Convert a price to a Y pixel coordinate.
    pub fn price_to_coordinate(&self, price: f64) -> f32 {
        // Use pane 0's price scale
        let pane = &self.inner.state.panes[0];
        pane.price_scale.price_to_y(price, &pane.layout_rect)
    }

    /// Convert a Y pixel coordinate to a price.
    pub fn coordinate_to_price(&self, y: f32) -> f64 {
        let pane = &self.inner.state.panes[0];
        pane.price_scale.y_to_price(y, &pane.layout_rect)
    }

    /// Convert a logical index to an X pixel coordinate.
    pub fn logical_to_coordinate(&self, logical: f64) -> f32 {
        unsafe {
            lumen_charts_core::chart_logical_to_coordinate(
                &self.inner as *const Chart as *mut Chart,
                logical,
            )
        }
    }

    /// Convert an X pixel coordinate to a logical index.
    pub fn coordinate_to_logical(&self, x: f32) -> f64 {
        unsafe {
            lumen_charts_core::chart_coordinate_to_logical(&self.inner as *const Chart as *mut Chart, x)
        }
    }

    // -- Sub-API accessors --

    /// Get the time scale API for this chart.
    pub fn time_scale(&mut self) -> TimeScaleApi<'_> {
        TimeScaleApi { chart: self }
    }

    /// Get the price scale API for a specific pane (default: pane 0).
    pub fn price_scale(&mut self, pane_index: u32) -> PriceScaleApi<'_> {
        PriceScaleApi {
            chart: self,
            pane_index,
        }
    }

    // -- Crosshair --

    /// Programmatically set the crosshair position.
    pub fn set_crosshair_position(&mut self, price: f64, time: i64, series: &SeriesApi) -> bool {
        unsafe {
            lumen_charts_core::chart_set_crosshair_position(
                &mut self.inner as *mut Chart,
                price,
                time,
                series.id,
            )
        }
    }

    /// Clear the crosshair position.
    pub fn clear_crosshair_position(&mut self) -> bool {
        unsafe { lumen_charts_core::chart_clear_crosshair_position(&mut self.inner as *mut Chart) }
    }

    // -- Formatting helpers --

    /// Format a price using the chart's localization settings.
    pub fn format_price(&self, price: f64) -> String {
        let ptr = unsafe { lumen_charts_core::chart_format_price(&self.inner as *const Chart, price) };
        if ptr.is_null() {
            return format!("{:.2}", price);
        }
        let s = unsafe { std::ffi::CStr::from_ptr(ptr).to_string_lossy().into_owned() };
        unsafe { lumen_charts_core::chart_free_string(ptr) };
        s
    }

    /// Format a timestamp as a date string.
    pub fn format_date(&self, timestamp: i64) -> String {
        let ptr =
            unsafe { lumen_charts_core::chart_format_date(&self.inner as *const Chart, timestamp) };
        if ptr.is_null() {
            return String::new();
        }
        let s = unsafe { std::ffi::CStr::from_ptr(ptr).to_string_lossy().into_owned() };
        unsafe { lumen_charts_core::chart_free_string(ptr) };
        s
    }

    /// Direct access to the inner `Chart` (for renderer access, egui integration, etc.).
    pub fn chart(&self) -> &Chart {
        &self.inner
    }

    /// Mutable access to the inner `Chart`.
    pub fn chart_mut(&mut self) -> &mut Chart {
        &mut self.inner
    }
}
