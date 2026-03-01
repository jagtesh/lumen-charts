import AppKit
import QuartzCore
import Metal
import CChartCore

// ---------------------------------------------------------------------------
// Sample data generation (ported from Rust sample_data.rs)
// ---------------------------------------------------------------------------

/// Generate ~100 bars of AAPL-like OHLC daily data as a flat [Double] array.
/// Format: [time, open, high, low, close, time, open, high, low, close, ...]
func generateSampleData() -> [Double] {
    let baseTime: Int64 = 1704153600  // 2024-01-02 00:00:00 UTC
    let day: Int64 = 86400
    var price: Double = 185.0
    var rng: UInt64 = 42
    var data: [Double] = []

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

        data.append(Double(time))
        data.append(open)
        data.append(max(high, max(open, close)))
        data.append(min(low, min(open, close)))
        data.append(close)

        price = close
    }
    return data
}

// ---------------------------------------------------------------------------
// Chart View
// ---------------------------------------------------------------------------

class ChartView: NSView {
    var metalLayer: CAMetalLayer?
    var chart: OpaquePointer?
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
        chart = chart_create(
            UInt32(size.width),
            UInt32(size.height),
            scaleFactor,
            layerPtr
        )

        // Load sample data via C-ABI
        var data = generateSampleData()
        chart_set_data(chart, &data, UInt32(data.count / 5))
        chart_fit_content(chart)
        chart_render(chart)

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
        if chart_pointer_move(chart, Float(p.x), Float(p.y)) { chart_render(chart) }
    }

    override func mouseDown(with event: NSEvent) {
        guard let chart = chart else { return }
        let p = convert(event.locationInWindow, from: nil)
        if chart_pointer_down(chart, Float(p.x), Float(p.y), 0) { chart_render(chart) }
    }

    override func mouseDragged(with event: NSEvent) {
        guard let chart = chart else { return }
        let p = convert(event.locationInWindow, from: nil)
        if chart_pointer_move(chart, Float(p.x), Float(p.y)) { chart_render(chart) }
    }

    override func mouseUp(with event: NSEvent) {
        guard let chart = chart else { return }
        let p = convert(event.locationInWindow, from: nil)
        if chart_pointer_up(chart, Float(p.x), Float(p.y), 0) { chart_render(chart) }
    }

    override func mouseExited(with event: NSEvent) {
        guard let chart = chart else { return }
        if chart_pointer_leave(chart) { chart_render(chart) }
    }

    override func scrollWheel(with event: NSEvent) {
        guard let chart = chart else { return }
        var needsRedraw = false
        if event.modifierFlags.contains(.command) || event.modifierFlags.contains(.control) {
            let factor: Float = 1.0 + Float(event.scrollingDeltaY) * 0.02
            let p = convert(event.locationInWindow, from: nil)
            needsRedraw = chart_zoom(chart, factor, Float(p.x))
        } else {
            needsRedraw = chart_scroll(chart, Float(-event.scrollingDeltaX), Float(event.scrollingDeltaY))
        }
        if needsRedraw { chart_render(chart) }
    }

    override func magnify(with event: NSEvent) {
        guard let chart = chart else { return }
        let p = convert(event.locationInWindow, from: nil)
        let factor = Float(1.0 + event.magnification)
        if chart_pinch(chart, factor, Float(p.x), Float(p.y)) { chart_render(chart) }
    }

    override func layout() {
        super.layout()
        guard let chart = chart else { return }
        let size = bounds.size
        if size.width > 0 && size.height > 0 {
            chart_resize(chart, UInt32(size.width), UInt32(size.height), scaleFactor)
            chart_render(chart)
        }
    }

    override func keyDown(with event: NSEvent) {
        guard let chart = chart else { return }
        let keyMap: [UInt16: UInt32] = [
            123: 37, 124: 39, 126: 38, 125: 40,
            24: 187, 27: 189, 115: 36, 119: 35,
        ]
        if let code = keyMap[event.keyCode] {
            if chart_key_down(chart, code) { chart_render(chart) }
        }
    }

    deinit {
        if let chart = chart { chart_destroy(chart) }
    }
}

// ---------------------------------------------------------------------------
// Toolbar
// ---------------------------------------------------------------------------

class ToolbarView: NSView {
    var onSeriesTypeChanged: ((UInt32) -> Void)?
    var onFitContent: (() -> Void)?
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
            labels: ["OHLC", "Candle", "Line"],
            trackingMode: .selectOne,
            target: self,
            action: #selector(seriesTypeChanged(_:))
        )
        typeControl.selectedSegment = 0
        typeControl.frame = NSRect(x: x, y: 4, width: 200, height: 24)
        typeControl.segmentStyle = .roundRect
        addSubview(typeControl)
        x += 210

        // Actions
        let fitBtn = makeButton("Fit Content", action: #selector(fitContentTapped))
        fitBtn.frame = NSRect(x: x, y: 4, width: 100, height: 24)
        addSubview(fitBtn)
        x += 110

        // Status
        statusLabel = makeLabel("OHLC Bars  •  100 bars from Swift")
        statusLabel.frame = NSRect(x: x, y: 6, width: 400, height: 20)
        statusLabel.textColor = NSColor(calibratedWhite: 0.5, alpha: 1.0)
        addSubview(statusLabel)
    }

    required init?(coder: NSCoder) { fatalError() }

    @objc func seriesTypeChanged(_ sender: NSSegmentedControl) {
        let names = ["OHLC Bars", "Candlestick", "Line"]
        statusLabel.stringValue = "\(names[sender.selectedSegment])  •  data from Swift"
        onSeriesTypeChanged?(UInt32(sender.selectedSegment))
    }

    @objc func fitContentTapped() { onFitContent?() }

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

    func applicationDidFinishLaunching(_ notification: Notification) {
        let rect = NSRect(x: 100, y: 100, width: 1000, height: 732)
        window = NSWindow(
            contentRect: rect,
            styleMask: [.titled, .closable, .resizable, .miniaturizable],
            backing: .buffered, defer: false
        )
        window.title = "Chart MVP — Rust Core + Swift Demo"
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

        toolbar.onSeriesTypeChanged = { [weak self] typeId in
            guard let chart = self?.chartView.chart else { return }
            if chart_set_series_type(chart, typeId) { chart_render(chart) }
        }

        toolbar.onFitContent = { [weak self] in
            guard let chart = self?.chartView.chart else { return }
            if chart_fit_content(chart) { chart_render(chart) }
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
