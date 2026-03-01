// Options.swift — Typed options structs for series and chart configuration
import Foundation

// MARK: - Color

/// RGBA color (0.0–1.0 per channel)
public struct ChartColor {
    public let r: Float
    public let g: Float
    public let b: Float
    public let a: Float

    public init(r: Float, g: Float, b: Float, a: Float = 1.0) {
        self.r = r; self.g = g; self.b = b; self.a = a
    }

    /// Create from hex string (#RGB, #RRGGBB, #RRGGBBAA)
    public init?(hex: String) {
        var s = hex.trimmingCharacters(in: .whitespaces)
        if s.hasPrefix("#") { s.removeFirst() }
        switch s.count {
        case 3:
            guard let rv = UInt8(s[s.startIndex...s.index(s.startIndex, offsetBy: 0)], radix: 16),
                  let gv = UInt8(s[s.index(s.startIndex, offsetBy: 1)...s.index(s.startIndex, offsetBy: 1)], radix: 16),
                  let bv = UInt8(s[s.index(s.startIndex, offsetBy: 2)...s.index(s.startIndex, offsetBy: 2)], radix: 16) else { return nil }
            self.r = Float(rv * 17) / 255.0
            self.g = Float(gv * 17) / 255.0
            self.b = Float(bv * 17) / 255.0
            self.a = 1.0
        case 6:
            guard let val = UInt32(s, radix: 16) else { return nil }
            self.r = Float((val >> 16) & 0xFF) / 255.0
            self.g = Float((val >> 8) & 0xFF) / 255.0
            self.b = Float(val & 0xFF) / 255.0
            self.a = 1.0
        case 8:
            guard let val = UInt32(s, radix: 16) else { return nil }
            self.r = Float((val >> 24) & 0xFF) / 255.0
            self.g = Float((val >> 16) & 0xFF) / 255.0
            self.b = Float((val >> 8) & 0xFF) / 255.0
            self.a = Float(val & 0xFF) / 255.0
        default:
            return nil
        }
    }

    /// Pack to UInt32 (RGBA, 8 bits per channel)
    public var rgba: UInt32 {
        let rv = UInt32(max(0, min(255, r * 255)))
        let gv = UInt32(max(0, min(255, g * 255)))
        let bv = UInt32(max(0, min(255, b * 255)))
        let av = UInt32(max(0, min(255, a * 255)))
        return (rv << 24) | (gv << 16) | (bv << 8) | av
    }

    // Common colors
    public static let white = ChartColor(r: 1, g: 1, b: 1)
    public static let black = ChartColor(r: 0, g: 0, b: 0)
    public static let blue = ChartColor(r: 0.15, g: 0.53, b: 0.99)
    public static let red = ChartColor(r: 0.94, g: 0.27, b: 0.27)
    public static let green = ChartColor(r: 0.16, g: 0.76, b: 0.49)
}

// MARK: - Series Options

public protocol SeriesOptionsProtocol {
    func toJSON() -> String
}

public struct LineSeriesOptions: SeriesOptionsProtocol {
    public var color: ChartColor
    public var lineWidth: Float
    public var visible: Bool

    public init(color: ChartColor = .blue, lineWidth: Float = 2, visible: Bool = true) {
        self.color = color; self.lineWidth = lineWidth; self.visible = visible
    }

    public func toJSON() -> String {
        "{\"color\":\"#\(hexString(color))\",\"lineWidth\":\(lineWidth),\"visible\":\(visible)}"
    }
}

public struct CandlestickSeriesOptions: SeriesOptionsProtocol {
    public var upColor: ChartColor
    public var downColor: ChartColor
    public var visible: Bool

    public init(upColor: ChartColor = .green, downColor: ChartColor = .red, visible: Bool = true) {
        self.upColor = upColor; self.downColor = downColor; self.visible = visible
    }

    public func toJSON() -> String {
        "{\"upColor\":\"#\(hexString(upColor))\",\"downColor\":\"#\(hexString(downColor))\",\"visible\":\(visible)}"
    }
}

public struct BarSeriesOptions: SeriesOptionsProtocol {
    public var upColor: ChartColor
    public var downColor: ChartColor
    public var visible: Bool

    public init(upColor: ChartColor = .green, downColor: ChartColor = .red, visible: Bool = true) {
        self.upColor = upColor; self.downColor = downColor; self.visible = visible
    }

    public func toJSON() -> String {
        "{\"upColor\":\"#\(hexString(upColor))\",\"downColor\":\"#\(hexString(downColor))\",\"visible\":\(visible)}"
    }
}

public struct AreaSeriesOptions: SeriesOptionsProtocol {
    public var lineColor: ChartColor
    public var visible: Bool

    public init(lineColor: ChartColor = .blue, visible: Bool = true) {
        self.lineColor = lineColor; self.visible = visible
    }

    public func toJSON() -> String {
        "{\"color\":\"#\(hexString(lineColor))\",\"visible\":\(visible)}"
    }
}

public struct HistogramSeriesOptions: SeriesOptionsProtocol {
    public var color: ChartColor
    public var visible: Bool

    public init(color: ChartColor = .blue, visible: Bool = true) {
        self.color = color; self.visible = visible
    }

    public func toJSON() -> String {
        "{\"color\":\"#\(hexString(color))\",\"visible\":\(visible)}"
    }
}

public struct BaselineSeriesOptions: SeriesOptionsProtocol {
    public var baseValue: Double
    public var topLineColor: ChartColor
    public var bottomLineColor: ChartColor
    public var visible: Bool

    public init(baseValue: Double = 0, topLineColor: ChartColor = .green,
                bottomLineColor: ChartColor = .red, visible: Bool = true) {
        self.baseValue = baseValue
        self.topLineColor = topLineColor; self.bottomLineColor = bottomLineColor
        self.visible = visible
    }

    public func toJSON() -> String {
        "{\"baseValue\":\(baseValue),\"visible\":\(visible)}"
    }
}

public struct PriceLineOptions {
    public var price: Double
    public var color: ChartColor
    public var lineWidth: Float
    public var title: String

    public init(price: Double, color: ChartColor = .white, lineWidth: Float = 1, title: String = "") {
        self.price = price; self.color = color
        self.lineWidth = lineWidth; self.title = title
    }

    func toJSON() -> String {
        let colorHex = hexString(color)
        let escaped = title.replacingOccurrences(of: "\"", with: "\\\"")
        return "{\"price\":\(price),\"color\":\"#\(colorHex)\",\"lineWidth\":\(lineWidth),\"title\":\"\(escaped)\"}"
    }
}

// MARK: - Helpers

private func hexString(_ c: ChartColor) -> String {
    let r = String(format: "%02x", Int(max(0, min(255, c.r * 255))))
    let g = String(format: "%02x", Int(max(0, min(255, c.g * 255))))
    let b = String(format: "%02x", Int(max(0, min(255, c.b * 255))))
    if c.a < 1.0 {
        let a = String(format: "%02x", Int(max(0, min(255, c.a * 255))))
        return r + g + b + a
    }
    return r + g + b
}
