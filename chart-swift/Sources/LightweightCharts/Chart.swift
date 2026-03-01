// Chart.swift — Main chart wrapper class
import CChartCore

/// A Lightweight Charts-compatible chart instance.
///
/// Usage:
/// ```swift
/// let chart = Chart(width: 800, height: 600, scaleFactor: 2.0, metalLayer: layerPtr)
/// chart.setData(ohlcBars)
/// chart.fitContent()
///
/// let line = chart.addLineSeries()
/// line.setData(linePoints)
/// ```
public class Chart {
    /// The underlying C-ABI opaque chart pointer
    public let ptr: OpaquePointer

    /// Track live series so we can clean them up
    private var liveSeries: [UInt32: SeriesAPI] = [:]

    /// Callback closures (stored to prevent deallocation)
    private var clickHandler: ((ChartEvent) -> Void)?
    private var dblClickHandler: ((ChartEvent) -> Void)?
    private var crosshairMoveHandler: ((ChartEvent) -> Void)?

    // MARK: - Lifecycle

    /// Create a new chart bound to a Metal layer.
    public init(width: UInt32, height: UInt32, scaleFactor: Double, metalLayer: UnsafeMutableRawPointer) {
        self.ptr = chart_create(width, height, scaleFactor, metalLayer)
    }

    deinit {
        chart_destroy(ptr)
    }

    // MARK: - Primary OHLC Data

    /// Set main OHLC data (the primary bar chart data)
    public func setData(_ bars: [OHLCData]) {
        // Pack into flat array: [time, open, high, low, close, ...]
        var flat: [Double] = []
        flat.reserveCapacity(bars.count * 5)
        for bar in bars {
            flat.append(Double(bar.time))
            flat.append(bar.open)
            flat.append(bar.high)
            flat.append(bar.low)
            flat.append(bar.close)
        }
        chart_set_data(ptr, &flat, UInt32(bars.count))
    }

    /// Update or append a single OHLC bar to the primary data
    @discardableResult
    public func updateBar(_ bar: OHLCData) -> Bool {
        chart_update_bar(ptr, bar.time, bar.open, bar.high, bar.low, bar.close)
    }

    /// Number of bars in primary data
    public var barCount: UInt32 {
        chart_bar_count(ptr)
    }

    // MARK: - Series Type (legacy primary series)

    /// Set the primary series display type
    public func setSeriesType(_ type: SeriesType) {
        let code: UInt32
        switch type {
        case .ohlc: code = 0
        case .candlestick: code = 1
        case .line: code = 2
        case .area: code = 3
        case .histogram: code = 4
        case .baseline: code = 5
        }
        if chart_set_series_type(ptr, code) { render() }
    }

    // MARK: - Add Series

    /// Add an OHLC bar series
    @discardableResult
    public func addBarSeries(data: [OHLCData] = [], options: BarSeriesOptions = .init()) -> SeriesAPI {
        var times = data.map { $0.time }
        var opens = data.map { $0.open }
        var highs = data.map { $0.high }
        var lows = data.map { $0.low }
        var closes = data.map { $0.close }
        let id = chart_add_ohlc_series(ptr, &times, &opens, &highs, &lows, &closes, UInt32(data.count))
        let series = SeriesAPI(id: id, seriesType: .ohlc, chartPtr: ptr)
        liveSeries[id] = series
        return series
    }

    /// Add a candlestick series
    @discardableResult
    public func addCandlestickSeries(data: [OHLCData] = [], options: CandlestickSeriesOptions = .init()) -> SeriesAPI {
        var times = data.map { $0.time }
        var opens = data.map { $0.open }
        var highs = data.map { $0.high }
        var lows = data.map { $0.low }
        var closes = data.map { $0.close }
        let id = chart_add_candlestick_series(ptr, &times, &opens, &highs, &lows, &closes, UInt32(data.count))
        let series = SeriesAPI(id: id, seriesType: .candlestick, chartPtr: ptr)
        liveSeries[id] = series
        return series
    }

    /// Add a line series
    @discardableResult
    public func addLineSeries(data: [LineData] = [], options: LineSeriesOptions = .init()) -> SeriesAPI {
        var times = data.map { $0.time }
        var values = data.map { $0.value }
        let id = chart_add_line_series(ptr, &times, &values, UInt32(data.count))
        let series = SeriesAPI(id: id, seriesType: .line, chartPtr: ptr)
        liveSeries[id] = series
        return series
    }

    /// Add an area series
    @discardableResult
    public func addAreaSeries(data: [LineData] = [], options: AreaSeriesOptions = .init()) -> SeriesAPI {
        var times = data.map { $0.time }
        var values = data.map { $0.value }
        let id = chart_add_area_series(ptr, &times, &values, UInt32(data.count))
        let series = SeriesAPI(id: id, seriesType: .area, chartPtr: ptr)
        liveSeries[id] = series
        return series
    }

    /// Add a baseline series
    @discardableResult
    public func addBaselineSeries(data: [LineData] = [], options: BaselineSeriesOptions = .init()) -> SeriesAPI {
        var times = data.map { $0.time }
        var values = data.map { $0.value }
        let id = chart_add_baseline_series(ptr, &times, &values, UInt32(data.count), options.baseValue)
        let series = SeriesAPI(id: id, seriesType: .baseline, chartPtr: ptr)
        liveSeries[id] = series
        return series
    }

    /// Add a histogram series
    @discardableResult
    public func addHistogramSeries(data: [HistogramData] = [], options: HistogramSeriesOptions = .init()) -> SeriesAPI {
        var times = data.map { $0.time }
        var values = data.map { $0.value }
        var colors = data.map { $0.color?.rgba ?? 0 }
        let id = chart_add_histogram_series(ptr, &times, &values, &colors, UInt32(data.count))
        let series = SeriesAPI(id: id, seriesType: .histogram, chartPtr: ptr)
        liveSeries[id] = series
        return series
    }

    /// Remove a series from the chart
    public func removeSeries(_ series: SeriesAPI) {
        chart_remove_series(ptr, series.id)
        liveSeries.removeValue(forKey: series.id)
    }

    /// Number of overlay series
    public var seriesCount: UInt32 {
        chart_series_count(ptr)
    }

    // MARK: - Panes

    /// Add a new pane
    @discardableResult
    public func addPane(heightStretch: Float = 1.0) -> PaneHandle {
        let id = chart_add_pane(ptr, heightStretch)
        return PaneHandle(id: id)
    }

    /// Remove a pane
    @discardableResult
    public func removePane(_ pane: PaneHandle) -> Bool {
        chart_remove_pane(ptr, pane.id)
    }

    /// Swap two panes
    @discardableResult
    public func swapPanes(_ a: PaneHandle, _ b: PaneHandle) -> Bool {
        chart_swap_panes(ptr, a.id, b.id)
    }

    /// Get pane count
    public var paneCount: UInt32 {
        chart_pane_count(ptr)
    }

    /// Get pane layout rect
    public func paneSize(_ pane: PaneHandle) -> (x: Float, y: Float, width: Float, height: Float)? {
        var x: Float = 0, y: Float = 0, w: Float = 0, h: Float = 0
        if chart_pane_size(ptr, pane.id, &x, &y, &w, &h) {
            return (x, y, w, h)
        }
        return nil
    }

    // MARK: - Rendering

    /// Render the chart (call after state changes if not auto-rendered)
    public func render() {
        chart_render(ptr)
    }

    /// Resize the chart
    public func resize(width: UInt32, height: UInt32, scaleFactor: Double) {
        chart_resize(ptr, width, height, scaleFactor)
    }

    /// Auto-fit visible content
    @discardableResult
    public func fitContent() -> Bool {
        let redraw = chart_fit_content(ptr)
        if redraw { render() }
        return redraw
    }

    // MARK: - Interactions

    @discardableResult
    public func pointerMove(x: Float, y: Float) -> Bool {
        let redraw = chart_pointer_move(ptr, x, y)
        if redraw { render() }
        return redraw
    }

    @discardableResult
    public func pointerDown(x: Float, y: Float, button: UInt8 = 0) -> Bool {
        let redraw = chart_pointer_down(ptr, x, y, button)
        if redraw { render() }
        return redraw
    }

    @discardableResult
    public func pointerUp(x: Float, y: Float, button: UInt8 = 0) -> Bool {
        let redraw = chart_pointer_up(ptr, x, y, button)
        if redraw { render() }
        return redraw
    }

    @discardableResult
    public func pointerLeave() -> Bool {
        let redraw = chart_pointer_leave(ptr)
        if redraw { render() }
        return redraw
    }

    @discardableResult
    public func scroll(deltaX: Float, deltaY: Float) -> Bool {
        let redraw = chart_scroll(ptr, deltaX, deltaY)
        if redraw { render() }
        return redraw
    }

    @discardableResult
    public func zoom(factor: Float, centerX: Float) -> Bool {
        let redraw = chart_zoom(ptr, factor, centerX)
        if redraw { render() }
        return redraw
    }

    @discardableResult
    public func pinch(scale: Float, centerX: Float, centerY: Float) -> Bool {
        let redraw = chart_pinch(ptr, scale, centerX, centerY)
        if redraw { render() }
        return redraw
    }

    @discardableResult
    public func keyDown(keyCode: UInt32) -> Bool {
        let redraw = chart_key_down(ptr, keyCode)
        if redraw { render() }
        return redraw
    }

    public func tick() {
        chart_tick(ptr)
    }

    // MARK: - Events

    /// Subscribe to click events
    public func subscribeClick(_ handler: @escaping (ChartEvent) -> Void) {
        clickHandler = handler
        let ctx = Unmanaged.passUnretained(self).toOpaque()
        chart_subscribe_click(ptr, { param, userData in
            guard let param = param, let userData = userData else { return }
            let chart = Unmanaged<Chart>.fromOpaque(userData).takeUnretainedValue()
            let event = ChartEvent(
                time: param.pointee.time,
                price: param.pointee.price,
                pointX: param.pointee.point_x,
                pointY: param.pointee.point_y,
                logical: param.pointee.logical
            )
            chart.clickHandler?(event)
        }, ctx)
    }

    /// Subscribe to double-click events
    public func subscribeDblClick(_ handler: @escaping (ChartEvent) -> Void) {
        dblClickHandler = handler
        let ctx = Unmanaged.passUnretained(self).toOpaque()
        chart_subscribe_dbl_click(ptr, { param, userData in
            guard let param = param, let userData = userData else { return }
            let chart = Unmanaged<Chart>.fromOpaque(userData).takeUnretainedValue()
            let event = ChartEvent(
                time: param.pointee.time,
                price: param.pointee.price,
                pointX: param.pointee.point_x,
                pointY: param.pointee.point_y,
                logical: param.pointee.logical
            )
            chart.dblClickHandler?(event)
        }, ctx)
    }

    /// Subscribe to crosshair move events
    public func subscribeCrosshairMove(_ handler: @escaping (ChartEvent) -> Void) {
        crosshairMoveHandler = handler
        let ctx = Unmanaged.passUnretained(self).toOpaque()
        chart_subscribe_crosshair_move(ptr, { param, userData in
            guard let param = param, let userData = userData else { return }
            let chart = Unmanaged<Chart>.fromOpaque(userData).takeUnretainedValue()
            let event = ChartEvent(
                time: param.pointee.time,
                price: param.pointee.price,
                pointX: param.pointee.point_x,
                pointY: param.pointee.point_y,
                logical: param.pointee.logical
            )
            chart.crosshairMoveHandler?(event)
        }, ctx)
    }

    /// Unsubscribe all event handlers
    public func unsubscribeAll() {
        chart_unsubscribe_click(ptr)
        chart_unsubscribe_dbl_click(ptr)
        chart_unsubscribe_crosshair_move(ptr)
        clickHandler = nil
        dblClickHandler = nil
        crosshairMoveHandler = nil
    }

    // MARK: - Crosshair

    /// Programmatically set crosshair position
    @discardableResult
    public func setCrosshairPosition(price: Double, time: Int64, seriesId: UInt32 = 0) -> Bool {
        let redraw = chart_set_crosshair_position(ptr, price, time, seriesId)
        if redraw { render() }
        return redraw
    }

    /// Clear the crosshair
    @discardableResult
    public func clearCrosshairPosition() -> Bool {
        let redraw = chart_clear_crosshair_position(ptr)
        if redraw { render() }
        return redraw
    }

    // MARK: - Coordinate Translation

    /// Convert a price value to a Y pixel coordinate
    public func priceToCoordinate(_ price: Double) -> Float {
        chart_price_to_coordinate(ptr, price)
    }

    /// Convert a Y pixel coordinate to a price value
    public func coordinateToPrice(_ y: Float) -> Double {
        chart_coordinate_to_price(ptr, y)
    }

    /// Convert a timestamp to an X pixel coordinate
    public func timeToCoordinate(_ time: Int64) -> Float {
        chart_time_to_coordinate(ptr, time)
    }

    /// Convert an X pixel coordinate to a timestamp
    public func coordinateToTime(_ x: Float) -> Int64 {
        chart_coordinate_to_time(ptr, x)
    }

    // MARK: - Options

    /// Apply chart-level options from JSON
    public func applyOptions(json: String) {
        json.withCString { cstr in
            chart_apply_options(ptr, cstr)
        }
    }
}
