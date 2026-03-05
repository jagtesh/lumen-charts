// DataTypes.swift — Swift data types mirroring Lightweight Charts
import Foundation

// MARK: - Bar Data

/// OHLC bar data (for Bar/Candlestick series)
public struct OHLCData {
    public let time: Int64
    public let open: Double
    public let high: Double
    public let low: Double
    public let close: Double

    public init(time: Int64, open: Double, high: Double, low: Double, close: Double) {
        self.time = time
        self.open = open
        self.high = high
        self.low = low
        self.close = close
    }
}

/// Single-value data point (for Line/Area/Baseline series)
public struct LineData {
    public let time: Int64
    public let value: Double

    public init(time: Int64, value: Double) {
        self.time = time
        self.value = value
    }
}

/// Histogram data point with optional per-bar color
public struct HistogramData {
    public let time: Int64
    public let value: Double
    public let color: ChartColor?

    public init(time: Int64, value: Double, color: ChartColor? = nil) {
        self.time = time
        self.value = value
        self.color = color
    }
}

// MARK: - Events

/// Event payload delivered to click/crosshair handlers.
///
/// # v5 `MouseEventParams` alignment
///
/// Includes `paneIndex` and `hoveredSeriesId` from v5's MouseEventParams.
/// Per-series data is available via Chart.eventSeriesData() (pull-based).
public struct ChartEvent {
    public let time: Int64
    public let price: Double
    public let pointX: Float
    public let pointY: Float
    public let logical: Double
    /// v5: Index of the pane where the event occurred
    public let paneIndex: UInt32
    /// v5: ID of the series under the cursor (0 = none)
    public let hoveredSeriesId: UInt32
    /// v5: Number of series with data at this position
    public let seriesCount: UInt32
}

// MARK: - Handles

/// Handle to a price line on a series
public struct PriceLine {
    public let id: UInt32
    let seriesId: UInt32
}

/// Handle to a chart pane (v5: index-based identity)
public struct PaneHandle {
    /// The pane index (not a persistent ID — shifts when panes are removed)
    public let index: UInt32
}

// MARK: - Series Type

/// Chart series types
public enum SeriesType {
    case ohlc
    case candlestick
    case line
    case area
    case histogram
    case baseline
}

/// v5 unified series definition — passed to `chart.addSeries()`.
///
/// Replaces per-type `addBarSeries()`, `addLineSeries()`, etc.
public enum SeriesDefinition {
    case ohlc
    case candlestick
    case line
    case area
    case histogram
    case baseline(baseValue: Double)

    /// Map to the corresponding SeriesType
    var seriesType: SeriesType {
        switch self {
        case .ohlc: return .ohlc
        case .candlestick: return .candlestick
        case .line: return .line
        case .area: return .area
        case .histogram: return .histogram
        case .baseline: return .baseline
        }
    }
}
