// SeriesAPI.swift — Type-safe wrapper for a chart series
import CChartCore

/// API for interacting with a chart series.
/// Returned from `Chart.addLineSeries()`, `Chart.addCandlestickSeries()`, etc.
public class SeriesAPI {
    /// The underlying C-ABI series ID
    public let id: UInt32

    /// The series type
    public let seriesType: SeriesType

    /// Weak reference to the parent chart pointer (unowned, chart must outlive series)
    let chartPtr: OpaquePointer

    init(id: UInt32, seriesType: SeriesType, chartPtr: OpaquePointer) {
        self.id = id
        self.seriesType = seriesType
        self.chartPtr = chartPtr
    }

    // MARK: - Data (OHLC)

    /// Set OHLC data for this series (Bar/Candlestick series)
    public func setData(_ data: [OHLCData]) {
        var times = data.map { $0.time }
        var opens = data.map { $0.open }
        var highs = data.map { $0.high }
        var lows = data.map { $0.low }
        var closes = data.map { $0.close }
        chart_series_set_ohlc_data(chartPtr, id, &times, &opens, &highs, &lows, &closes, UInt32(data.count))
    }

    /// Set single-value data for this series (Line/Area/Baseline series)
    public func setData(_ data: [LineData]) {
        var times = data.map { $0.time }
        var values = data.map { $0.value }
        chart_series_set_line_data(chartPtr, id, &times, &values, UInt32(data.count))
    }

    /// Set histogram data for this series
    public func setData(_ data: [HistogramData]) {
        var times = data.map { $0.time }
        var values = data.map { $0.value }
        var colors = data.map { $0.color?.rgba ?? 0 }
        chart_series_set_histogram_data(chartPtr, id, &times, &values, &colors, UInt32(data.count))
    }

    // MARK: - Update

    /// Update or append a single OHLC bar
    public func update(_ bar: OHLCData) {
        chart_series_update_ohlc_bar(chartPtr, id, bar.time, bar.open, bar.high, bar.low, bar.close)
    }

    /// Update or append a single line/area/baseline point
    public func update(_ point: LineData) {
        chart_series_update_line_bar(chartPtr, id, point.time, point.value)
    }

    /// Update or append a single histogram point
    public func update(_ point: HistogramData) {
        chart_series_update_histogram_bar(
            chartPtr, id, point.time, point.value,
            point.color?.rgba ?? 0, point.color != nil
        )
    }

    /// Remove the last `count` data points
    public func pop(count: UInt32 = 1) {
        chart_series_pop(chartPtr, id, count)
    }

    // MARK: - Options

    /// Apply options to this series
    public func applyOptions(_ options: SeriesOptionsProtocol) {
        let json = options.toJSON()
        json.withCString { cstr in
            chart_series_apply_options(chartPtr, id, cstr)
        }
    }

    // MARK: - Price Lines

    /// Create a price line on this series
    @discardableResult
    public func createPriceLine(options: PriceLineOptions) -> PriceLine {
        let json = options.toJSON()
        let lineId = json.withCString { cstr -> UInt32 in
            chart_series_create_price_line(chartPtr, id, cstr)
        }
        return PriceLine(id: lineId, seriesId: id)
    }

    /// Remove a price line from this series
    public func removePriceLine(_ line: PriceLine) {
        chart_series_remove_price_line(chartPtr, id, line.id)
    }

    // MARK: - Pane (v5: index-based)

    /// Move this series to a different pane
    public func moveToPane(_ pane: PaneHandle) {
        chart_series_move_to_pane(chartPtr, id, pane.index)
    }

    /// Get the pane index this series is assigned to (v5: ISeriesApi.getPane)
    public var paneIndex: UInt32 {
        chart_series_get_pane_index(chartPtr, id)
    }

    /// Get a PaneHandle for the pane this series belongs to
    public func getPane() -> PaneHandle {
        PaneHandle(index: paneIndex)
    }

    /// Get the z-order of this series within its pane (v5: seriesOrder)
    public func seriesOrder() -> UInt32 {
        chart_series_order(chartPtr, id)
    }

    /// Set the z-order of this series within its pane (v5: setSeriesOrder)
    @discardableResult
    public func setSeriesOrder(_ order: UInt32) -> Bool {
        chart_series_set_order(chartPtr, id, order)
    }

    /// Number of data points in this series
    public var dataLength: UInt32 {
        chart_series_data_length(chartPtr, id)
    }

    /// Get the last value data (time + value)
    public var lastValueData: (time: Int64, value: Double)? {
        var time: Int64 = 0
        var value: Double = 0
        if chart_series_get_last_value_data(chartPtr, id, &time, &value) {
            return (time, value)
        }
        return nil
    }

    /// Get the C-ABI series type code for this series
    public var typeCode: UInt32 {
        chart_series_type(chartPtr, id)
    }

    /// Get data at a specific index
    public func dataByIndex(_ index: Int32) -> (time: Int64, value: Double)? {
        var time: Int64 = 0
        var value: Double = 0
        if chart_data_by_index(chartPtr, id, index, &time, &value) {
            return (time, value)
        }
        return nil
    }
}
