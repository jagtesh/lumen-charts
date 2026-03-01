import AppKit
import QuartzCore
import Metal
import LightweightCharts

// ---------------------------------------------------------------------------
// Sample data generation (ported from Rust sample_data.rs)
// ---------------------------------------------------------------------------

/// Generate ~100 bars of AAPL-like OHLC daily data
func generateSampleData() -> [OHLCData] {
    let baseTime: Int64 = 1704153600  // 2024-01-02 00:00:00 UTC
    let day: Int64 = 86400
    var price: Double = 185.0
    var rng: UInt64 = 42
    var bars: [OHLCData] = []

    func nextRand() -> Double {
        rng = rng &* 6364136223846793005 &+ 1442695040888963407
        return Double(rng >> 33) / Double(UInt64(1) << 31) * 2.0 - 1.0
    }

    for i in 0..<100 {
        let time = baseTime + Int64(i) * day
        let changePct = nextRand() * 0.02
        let dailyRange = price * (0.005 + abs(nextRand()) * 0.015)

        let open = price
        let close = price * (1.0 + changePct)
        let high = max(open, close) + dailyRange * abs(nextRand())
        let low = min(open, close) - dailyRange * abs(nextRand())

        bars.append(OHLCData(
            time: time,
            open: open,
            high: max(high, max(open, close)),
            low: min(low, min(open, close)),
            close: close
        ))
        price = close
    }
    return bars
}

/// Generate line data from OHLC (closing prices offset down, simulating a moving average)
func generateOverlayData() -> [LineData] {
    generateSampleData().map { LineData(time: $0.time, value: $0.close - 15.0) }
}

// MARK: - MACD Indicator Calculation

/// Exponential Moving Average
func ema(values: [Double], period: Int) -> [Double] {
    guard values.count >= period else { return values }
    let k = 2.0 / Double(period + 1)
    var result = [Double](repeating: 0.0, count: values.count)
    // SMA for the first `period` values
    let sma = values.prefix(period).reduce(0.0, +) / Double(period)
    result[period - 1] = sma
    for i in period..<values.count {
        result[i] = values[i] * k + result[i - 1] * (1.0 - k)
    }
    return result
}

struct MACDResult {
    var macdLine: [LineData]
    var signalLine: [LineData]
    var histogram: [HistogramData]
}

/// Calculate MACD (12, 26, 9) from OHLC data
func calculateMACD(bars: [OHLCData]) -> MACDResult {
    let closes = bars.map { $0.close }
    let ema12 = ema(values: closes, period: 12)
    let ema26 = ema(values: closes, period: 26)

    // MACD line = EMA(12) - EMA(26), valid from index 25 onward
    let startIdx = 25
    var macdValues: [Double] = []
    var macdTimes: [Int64] = []
    for i in startIdx..<bars.count {
        macdValues.append(ema12[i] - ema26[i])
        macdTimes.append(bars[i].time)
    }

    // Signal line = EMA(9) of MACD
    let signalValues = ema(values: macdValues, period: 9)
    let signalStart = 8 // signal valid from index 8 within macdValues

    var macdLine: [LineData] = []
    var signalLine: [LineData] = []
    var histogram: [HistogramData] = []

    for i in signalStart..<macdValues.count {
        let time = macdTimes[i]
        let macd = macdValues[i]
        let signal = signalValues[i]
        let hist = macd - signal

        macdLine.append(LineData(time: time, value: macd))
        signalLine.append(LineData(time: time, value: signal))

        // Green when histogram is positive, red when negative
        let color: ChartColor = hist >= 0
            ? ChartColor(r: 0.16, g: 0.76, b: 0.49, a: 0.8)
            : ChartColor(r: 0.94, g: 0.27, b: 0.27, a: 0.8)
        histogram.append(HistogramData(time: time, value: hist, color: color))
    }

    return MACDResult(macdLine: macdLine, signalLine: signalLine, histogram: histogram)
}

// ---------------------------------------------------------------------------
// Chart View
// ---------------------------------------------------------------------------

class ChartView: NSView {
    var metalLayer: CAMetalLayer?
    var chart: Chart?
    var trackingArea: NSTrackingArea?
    var scaleFactor: Double = 2.0

    override var wantsLayer: Bool { get { true } set {} }
    override var isFlipped: Bool { true }

    override func viewDidMoveToWindow() {
        super.viewDidMoveToWindow()
        guard let _ = window else { return }
        guard chart == nil else { return }

        let ml = CAMetalLayer()
        ml.device = MTLCreateSystemDefaultDevice()
        ml.pixelFormat = .bgra8Unorm
        ml.framebufferOnly = true
        scaleFactor = Double(NSScreen.main?.backingScaleFactor ?? 2.0)
        ml.contentsScale = CGFloat(scaleFactor)
        self.metalLayer = ml
        self.layer = ml

        let layerPtr = Unmanaged.passUnretained(ml).toOpaque()
        let size = bounds.size

        // Create chart via wrapper API
        let c = Chart(
            width: UInt32(size.width),
            height: UInt32(size.height),
            scaleFactor: scaleFactor,
            metalLayer: layerPtr
        )

        // Load sample data
        c.setData(generateSampleData())
        c.fitContent()
        c.render()

        self.chart = c
        updateTrackingArea()
    }

    func updateTrackingArea() {
        if let existing = trackingArea { removeTrackingArea(existing) }
        trackingArea = NSTrackingArea(
            rect: bounds,
            options: [.mouseMoved, .mouseEnteredAndExited, .activeInKeyWindow, .inVisibleRect],
            owner: self,
            userInfo: nil
        )
        addTrackingArea(trackingArea!)
    }

    override var acceptsFirstResponder: Bool { true }

    override func mouseMoved(with event: NSEvent) {
        guard let chart = chart else { return }
        let p = convert(event.locationInWindow, from: nil)
        chart.pointerMove(x: Float(p.x), y: Float(p.y))
    }

    override func mouseDown(with event: NSEvent) {
        guard let chart = chart else { return }
        let p = convert(event.locationInWindow, from: nil)
        chart.pointerDown(x: Float(p.x), y: Float(p.y))
    }

    override func mouseDragged(with event: NSEvent) {
        guard let chart = chart else { return }
        let p = convert(event.locationInWindow, from: nil)
        chart.pointerMove(x: Float(p.x), y: Float(p.y))
    }

    override func mouseUp(with event: NSEvent) {
        guard let chart = chart else { return }
        let p = convert(event.locationInWindow, from: nil)
        chart.pointerUp(x: Float(p.x), y: Float(p.y))
    }

    override func mouseExited(with event: NSEvent) {
        chart?.pointerLeave()
    }

    override func scrollWheel(with event: NSEvent) {
        guard let chart = chart else { return }
        if event.modifierFlags.contains(.command) || event.modifierFlags.contains(.control) {
            let factor: Float = 1.0 + Float(event.scrollingDeltaY) * 0.02
            let p = convert(event.locationInWindow, from: nil)
            chart.zoom(factor: factor, centerX: Float(p.x))
        } else {
            chart.scroll(deltaX: Float(-event.scrollingDeltaX), deltaY: Float(event.scrollingDeltaY))
        }
    }

    override func magnify(with event: NSEvent) {
        guard let chart = chart else { return }
        let p = convert(event.locationInWindow, from: nil)
        chart.pinch(scale: Float(1.0 + event.magnification), centerX: Float(p.x), centerY: Float(p.y))
    }

    override func layout() {
        super.layout()
        guard let chart = chart else { return }
        let size = bounds.size
        if size.width > 0 && size.height > 0 {
            chart.resize(width: UInt32(size.width), height: UInt32(size.height), scaleFactor: scaleFactor)
            chart.render()
        }
    }

    override func keyDown(with event: NSEvent) {
        guard let chart = chart else { return }
        let keyMap: [UInt16: UInt32] = [
            123: 37, 124: 39, 126: 38, 125: 40,
            24: 187, 27: 189, 115: 36, 119: 35,
        ]
        if let code = keyMap[event.keyCode] {
            chart.keyDown(keyCode: code)
        }
    }
}

// ---------------------------------------------------------------------------
// Toolbar
// ---------------------------------------------------------------------------

class ToolbarView: NSView {
    var onSeriesTypeChanged: ((SeriesType) -> Void)?
    var onFitContent: (() -> Void)?
    var onToggleOverlay: (() -> Void)?
    var onToggleMACD: (() -> Void)?
    var statusLabel: NSTextField!

    override init(frame: NSRect) {
        super.init(frame: frame)
        wantsLayer = true
        layer?.backgroundColor = NSColor(calibratedWhite: 0.12, alpha: 1.0).cgColor

        var x: CGFloat = 12

        // Chart type
        let typeLabel = makeLabel("Chart Type:")
        typeLabel.frame = NSRect(x: x, y: 6, width: 80, height: 20)
        addSubview(typeLabel)
        x += 80

        let typeControl = NSSegmentedControl(
            labels: ["OHLC", "Candle", "Line", "Area", "Hist", "Base"],
            trackingMode: .selectOne,
            target: self,
            action: #selector(seriesTypeChanged(_:))
        )
        typeControl.selectedSegment = 0
        typeControl.frame = NSRect(x: x, y: 4, width: 330, height: 24)
        typeControl.segmentStyle = .roundRect
        addSubview(typeControl)
        x += 340

        // Actions
        let fitBtn = makeButton("Fit Content", action: #selector(fitContentTapped))
        fitBtn.frame = NSRect(x: x, y: 4, width: 90, height: 24)
        addSubview(fitBtn)
        x += 100

        let overlayBtn = makeButton("Toggle Overlay", action: #selector(toggleOverlayTapped))
        overlayBtn.frame = NSRect(x: x, y: 4, width: 110, height: 24)
        addSubview(overlayBtn)
        x += 120

        let macdBtn = makeButton("Toggle MACD", action: #selector(toggleMACDTapped))
        macdBtn.frame = NSRect(x: x, y: 4, width: 110, height: 24)
        addSubview(macdBtn)
        x += 120

        // Status
        statusLabel = makeLabel("OHLC Bars  •  100 bars from Swift")
        statusLabel.frame = NSRect(x: x, y: 6, width: 300, height: 20)
        statusLabel.textColor = NSColor(calibratedWhite: 0.5, alpha: 1.0)
        addSubview(statusLabel)
    }

    required init?(coder: NSCoder) { fatalError() }

    @objc func seriesTypeChanged(_ sender: NSSegmentedControl) {
        let types: [SeriesType] = [.ohlc, .candlestick, .line, .area, .histogram, .baseline]
        let names = ["OHLC Bars", "Candlestick", "Line", "Area", "Histogram", "Baseline"]
        statusLabel.stringValue = "\(names[sender.selectedSegment])  •  data from Swift"
        onSeriesTypeChanged?(types[sender.selectedSegment])
    }

    @objc func fitContentTapped() { onFitContent?() }
    @objc func toggleOverlayTapped() { onToggleOverlay?() }
    @objc func toggleMACDTapped() { onToggleMACD?() }

    private func makeLabel(_ text: String) -> NSTextField {
        let l = NSTextField(labelWithString: text)
        l.font = NSFont.systemFont(ofSize: 12, weight: .medium)
        l.textColor = NSColor(calibratedWhite: 0.7, alpha: 1.0)
        return l
    }

    private func makeButton(_ title: String, action: Selector) -> NSButton {
        let b = NSButton(title: title, target: self, action: action)
        b.bezelStyle = .rounded
        b.font = NSFont.systemFont(ofSize: 11)
        return b
    }
}

// ---------------------------------------------------------------------------
// App Delegate
// ---------------------------------------------------------------------------

class AppDelegate: NSObject, NSApplicationDelegate {
    var window: NSWindow!
    var chartView: ChartView!
    var toolbar: ToolbarView!
    let toolbarHeight: CGFloat = 32
    var overlaySeries: SeriesAPI?
    var macdPane: PaneHandle?
    var macdSeries: [SeriesAPI] = []

    func applicationDidFinishLaunching(_ notification: Notification) {
        let rect = NSRect(x: 100, y: 100, width: 1000, height: 732)
        window = NSWindow(
            contentRect: rect,
            styleMask: [.titled, .closable, .resizable, .miniaturizable],
            backing: .buffered, defer: false
        )
        window.title = "Chart MVP — Rust Core + Swift Wrapper"
        window.center()
        window.minSize = NSSize(width: 600, height: 400)

        let container = NSView(frame: rect)
        container.autoresizesSubviews = true

        toolbar = ToolbarView(frame: NSRect(
            x: 0, y: rect.height - toolbarHeight,
            width: rect.width, height: toolbarHeight
        ))
        toolbar.autoresizingMask = [.width, .minYMargin]
        container.addSubview(toolbar)

        chartView = ChartView(frame: NSRect(
            x: 0, y: 0,
            width: rect.width, height: rect.height - toolbarHeight
        ))
        chartView.autoresizingMask = [.width, .height]
        container.addSubview(chartView)

        window.contentView = container

        // Series type switching — using the wrapper API
        toolbar.onSeriesTypeChanged = { [weak self] type in
            self?.chartView.chart?.setSeriesType(type)
        }

        // Fit content
        toolbar.onFitContent = { [weak self] in
            self?.chartView.chart?.fitContent()
        }

        // Toggle overlay — using the wrapper API
        toolbar.onToggleOverlay = { [weak self] in
            guard let self = self, let chart = self.chartView.chart else { return }
            if let series = self.overlaySeries {
                // Remove existing overlay
                chart.removeSeries(series)
                self.overlaySeries = nil
                chart.render()
            } else {
                // Add area series overlay with data
                let series = chart.addAreaSeries(data: generateOverlayData())
                self.overlaySeries = series
                chart.render()
            }
        }

        // Toggle MACD pane
        toolbar.onToggleMACD = { [weak self] in
            guard let self = self, let chart = self.chartView.chart else { return }
            if let pane = self.macdPane {
                // Remove all MACD series, then remove the pane
                for s in self.macdSeries { chart.removeSeries(s) }
                self.macdSeries.removeAll()
                chart.removePane(pane)
                self.macdPane = nil
                chart.render()
            } else {
                // Add MACD pane
                let pane = chart.addPane(heightStretch: 0.3)
                self.macdPane = pane

                let macd = calculateMACD(bars: generateSampleData())

                // Histogram
                let histSeries = chart.addHistogramSeries(data: macd.histogram)
                histSeries.moveToPane(pane)

                // MACD line (blue)
                let macdLineSeries = chart.addLineSeries(
                    data: macd.macdLine,
                    options: LineSeriesOptions(color: ChartColor(r: 0.2, g: 0.6, b: 1.0), lineWidth: 1.5)
                )
                macdLineSeries.moveToPane(pane)

                // Signal line (orange)
                let signalSeries = chart.addLineSeries(
                    data: macd.signalLine,
                    options: LineSeriesOptions(color: ChartColor(r: 1.0, g: 0.6, b: 0.2), lineWidth: 1.5)
                )
                signalSeries.moveToPane(pane)

                self.macdSeries = [histSeries, macdLineSeries, signalSeries]
                chart.render()
            }
        }

        window.makeKeyAndOrderFront(nil)
        window.makeFirstResponder(chartView)
    }
}

// --- Main ---
let app = NSApplication.shared
app.setActivationPolicy(.regular)
let delegate = AppDelegate()
app.delegate = delegate
app.run()
