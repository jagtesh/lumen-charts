import AppKit
import QuartzCore
import CChartCore

// ---------------------------------------------------------------------------
// MARK: - ChartView (NSView backed by CAMetalLayer)
// ---------------------------------------------------------------------------

class ChartView: NSView {
    private var chart: OpaquePointer?
    private var metalLayer: CAMetalLayer!

    override init(frame: NSRect) {
        super.init(frame: frame)
        wantsLayer = true
    }

    required init?(coder: NSCoder) {
        super.init(coder: coder)
        wantsLayer = true
    }

    override func makeBackingLayer() -> CALayer {
        metalLayer = CAMetalLayer()
        metalLayer.device = MTLCreateSystemDefaultDevice()
        metalLayer.pixelFormat = .bgra8Unorm
        metalLayer.framebufferOnly = true
        metalLayer.contentsScale = NSScreen.main?.backingScaleFactor ?? 2.0
        return metalLayer
    }

    override func viewDidMoveToWindow() {
        super.viewDidMoveToWindow()
        guard let window = window else { return }

        let scaleFactor = window.backingScaleFactor
        metalLayer.contentsScale = scaleFactor

        // Create the Rust chart, passing the CAMetalLayer pointer
        let layerPtr = Unmanaged.passUnretained(metalLayer).toOpaque()
        let size = bounds.size
        chart = chart_create(
            UInt32(size.width),
            UInt32(size.height),
            Double(scaleFactor),
            layerPtr
        )

        // Initial render
        render()
    }

    override func setFrameSize(_ newSize: NSSize) {
        super.setFrameSize(newSize)

        guard let chart = chart else { return }
        let scaleFactor = window?.backingScaleFactor ?? 2.0
        metalLayer.contentsScale = scaleFactor
        metalLayer.drawableSize = CGSize(
            width: newSize.width * scaleFactor,
            height: newSize.height * scaleFactor
        )

        chart_resize(chart, UInt32(newSize.width), UInt32(newSize.height), Double(scaleFactor))
        render()
    }

    func render() {
        guard let chart = chart else { return }
        chart_render(chart)
    }

    deinit {
        if let chart = chart {
            chart_destroy(chart)
        }
    }
}

// ---------------------------------------------------------------------------
// MARK: - App Delegate
// ---------------------------------------------------------------------------

class AppDelegate: NSObject, NSApplicationDelegate {
    var window: NSWindow!
    var chartView: ChartView!

    func applicationDidFinishLaunching(_ notification: Notification) {
        let windowRect = NSRect(x: 100, y: 100, width: 900, height: 600)
        window = NSWindow(
            contentRect: windowRect,
            styleMask: [.titled, .closable, .resizable, .miniaturizable],
            backing: .buffered,
            defer: false
        )
        window.title = "Chart MVP — Rust Core + Swift Demo"
        window.minSize = NSSize(width: 400, height: 300)

        chartView = ChartView(frame: windowRect)
        window.contentView = chartView
        window.makeKeyAndOrderFront(nil)
    }

    func applicationShouldTerminateAfterLastWindowClosed(_ sender: NSApplication) -> Bool {
        return true
    }
}

// ---------------------------------------------------------------------------
// MARK: - Entry Point
// ---------------------------------------------------------------------------

let app = NSApplication.shared
let delegate = AppDelegate()
app.delegate = delegate
app.run()
