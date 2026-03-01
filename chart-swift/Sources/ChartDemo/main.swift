import AppKit
import QuartzCore
import Metal
import CChartCore

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
        guard chart == nil else { return } // Already initialized

        // Create Metal layer eagerly
        let ml = CAMetalLayer()
        ml.device = MTLCreateSystemDefaultDevice()
        ml.pixelFormat = .bgra8Unorm
        ml.framebufferOnly = true
        scaleFactor = Double(NSScreen.main?.backingScaleFactor ?? 2.0)
        ml.contentsScale = CGFloat(scaleFactor)
        self.metalLayer = ml

        // Set as the view's layer
        self.layer = ml

        let layerPtr = Unmanaged.passUnretained(ml).toOpaque()
        let size = bounds.size
        chart = chart_create(
            UInt32(size.width),
            UInt32(size.height),
            scaleFactor,
            layerPtr
        )

        // Initial render
        chart_render(chart)

        // Setup tracking area for mouse events
        updateTrackingArea()
    }


    func updateTrackingArea() {
        if let existing = trackingArea {
            removeTrackingArea(existing)
        }
        trackingArea = NSTrackingArea(
            rect: bounds,
            options: [.mouseMoved, .mouseEnteredAndExited, .activeInKeyWindow, .inVisibleRect],
            owner: self,
            userInfo: nil
        )
        addTrackingArea(trackingArea!)
    }

    override var acceptsFirstResponder: Bool { true }

    // --- Mouse move → crosshair ---
    override func mouseMoved(with event: NSEvent) {
        guard let chart = chart else { return }
        let p = convert(event.locationInWindow, from: nil)
        if chart_pointer_move(chart, Float(p.x), Float(p.y)) {
            chart_render(chart)
        }
    }

    override func mouseDown(with event: NSEvent) {
        guard let chart = chart else { return }
        let p = convert(event.locationInWindow, from: nil)
        if chart_pointer_down(chart, Float(p.x), Float(p.y), 0) {
            chart_render(chart)
        }
    }

    // --- Mouse drag → pan ---
    override func mouseDragged(with event: NSEvent) {
        guard let chart = chart else { return }
        let p = convert(event.locationInWindow, from: nil)
        if chart_pointer_move(chart, Float(p.x), Float(p.y)) {
            chart_render(chart)
        }
    }

    // --- Mouse up → drag end ---
    override func mouseUp(with event: NSEvent) {
        guard let chart = chart else { return }
        let p = convert(event.locationInWindow, from: nil)
        if chart_pointer_up(chart, Float(p.x), Float(p.y), 0) {
            chart_render(chart)
        }
    }

    // --- Mouse exited → hide crosshair ---
    override func mouseExited(with event: NSEvent) {
        guard let chart = chart else { return }
        if chart_pointer_leave(chart) {
            chart_render(chart)
        }
    }

    // --- Scroll wheel → pan or zoom ---
    override func scrollWheel(with event: NSEvent) {
        guard let chart = chart else { return }
        var needsRedraw = false

        if event.modifierFlags.contains(.command) || event.modifierFlags.contains(.control) {
            // Cmd/Ctrl + scroll = zoom
            let factor: Float = 1.0 + Float(event.scrollingDeltaY) * 0.02
            let p = convert(event.locationInWindow, from: nil)
            needsRedraw = chart_zoom(chart, factor, Float(p.x))
        } else {
            // Regular scroll = horizontal pan
            needsRedraw = chart_scroll(chart, Float(-event.scrollingDeltaX), Float(event.scrollingDeltaY))
        }

        if needsRedraw {
            chart_render(chart)
        }
    }

    // --- Magnify gesture (trackpad pinch) ---
    override func magnify(with event: NSEvent) {
        guard let chart = chart else { return }
        let p = convert(event.locationInWindow, from: nil)
        let factor = Float(1.0 + event.magnification)
        if chart_pinch(chart, factor, Float(p.x), Float(p.y)) {
            chart_render(chart)
        }
    }


    // --- Resize ---
    override func layout() {
        super.layout()
        guard let chart = chart else { return }
        let size = bounds.size
        if size.width > 0 && size.height > 0 {
            chart_resize(chart, UInt32(size.width), UInt32(size.height), scaleFactor)
            chart_render(chart)
        }
    }

    // --- Keyboard ---
    override func keyDown(with event: NSEvent) {
        guard let chart = chart else { return }
        let keyMap: [UInt16: UInt32] = [
            123: 37,  // ArrowLeft
            124: 39,  // ArrowRight
            126: 38,  // ArrowUp
            125: 40,  // ArrowDown
            24: 187,  // + key
            27: 189,  // - key
            115: 36,  // Home
            119: 35,  // End
        ]
        if let code = keyMap[event.keyCode] {
            if chart_key_down(chart, code) {
                chart_render(chart)
            }
        }
    }

    deinit {
        if let chart = chart {
            chart_destroy(chart)
        }
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

        // --- Series Type Segmented Control ---
        let seriesLabel = makeLabel("Chart Type:")
        seriesLabel.frame = NSRect(x: 12, y: 6, width: 80, height: 20)
        addSubview(seriesLabel)

        let seriesControl = NSSegmentedControl(labels: ["OHLC", "Candle", "Line"], trackingMode: .selectOne, target: self, action: #selector(seriesTypeChanged(_:)))
        seriesControl.selectedSegment = 0
        seriesControl.frame = NSRect(x: 92, y: 4, width: 200, height: 24)
        seriesControl.segmentStyle = .roundRect
        addSubview(seriesControl)

        // --- Actions ---
        let fitBtn = makeButton("Fit Content", action: #selector(fitContentTapped))
        fitBtn.frame = NSRect(x: 310, y: 4, width: 100, height: 24)
        addSubview(fitBtn)

        // --- Status label ---
        statusLabel = makeLabel("OHLC Bars")
        statusLabel.frame = NSRect(x: 430, y: 6, width: 300, height: 20)
        statusLabel.textColor = NSColor(calibratedWhite: 0.5, alpha: 1.0)
        addSubview(statusLabel)
    }

    required init?(coder: NSCoder) { fatalError() }

    @objc func seriesTypeChanged(_ sender: NSSegmentedControl) {
        let typeNames = ["OHLC Bars", "Candlestick", "Line"]
        statusLabel.stringValue = typeNames[sender.selectedSegment]
        onSeriesTypeChanged?(UInt32(sender.selectedSegment))
    }

    @objc func fitContentTapped() {
        onFitContent?()
    }

    private func makeLabel(_ text: String) -> NSTextField {
        let label = NSTextField(labelWithString: text)
        label.font = NSFont.systemFont(ofSize: 12, weight: .medium)
        label.textColor = NSColor(calibratedWhite: 0.7, alpha: 1.0)
        return label
    }

    private func makeButton(_ title: String, action: Selector) -> NSButton {
        let btn = NSButton(title: title, target: self, action: action)
        btn.bezelStyle = .rounded
        btn.font = NSFont.systemFont(ofSize: 11)
        return btn
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
        let windowRect = NSRect(x: 100, y: 100, width: 1000, height: 732)
        window = NSWindow(
            contentRect: windowRect,
            styleMask: [.titled, .closable, .resizable, .miniaturizable],
            backing: .buffered,
            defer: false
        )
        window.title = "Chart MVP — Rust Core + Swift Demo"
        window.center()
        window.minSize = NSSize(width: 600, height: 400)

        // Container
        let container = NSView(frame: windowRect)
        container.autoresizesSubviews = true

        // Toolbar at top
        toolbar = ToolbarView(frame: NSRect(
            x: 0,
            y: windowRect.height - toolbarHeight,
            width: windowRect.width,
            height: toolbarHeight
        ))
        toolbar.autoresizingMask = [.width, .minYMargin]
        container.addSubview(toolbar)

        // Chart fills the rest
        chartView = ChartView(frame: NSRect(
            x: 0,
            y: 0,
            width: windowRect.width,
            height: windowRect.height - toolbarHeight
        ))
        chartView.autoresizingMask = [.width, .height]
        container.addSubview(chartView)

        window.contentView = container

        // Wire up toolbar actions
        toolbar.onSeriesTypeChanged = { [weak self] typeId in
            guard let chart = self?.chartView.chart else { return }
            if chart_set_series_type(chart, typeId) {
                chart_render(chart)
            }
        }

        toolbar.onFitContent = { [weak self] in
            guard let chart = self?.chartView.chart else { return }
            if chart_fit_content(chart) {
                chart_render(chart)
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
