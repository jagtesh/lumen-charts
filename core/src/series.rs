use crate::chart_model::OhlcBar;
use crate::draw_backend::{Color, ColorName};
use serde::{Deserialize, Serialize};

// ---------------------------------------------------------------------------
// Series types
// ---------------------------------------------------------------------------

/// The type of a series
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum SeriesType {
    /// OHLC bars (current default renderer)
    Ohlc,
    /// Candlestick (filled/hollow bodies with wicks)
    Candlestick,
    /// Line series (connects close prices)
    Line,
    /// Area series (line + gradient fill below)
    Area,
    /// Histogram series (vertical bars from baseline)
    Histogram,
    /// Baseline series (line with top/bottom areas split at a baseline value)
    Baseline,
}

impl Default for SeriesType {
    fn default() -> Self {
        SeriesType::Ohlc
    }
}

// ---------------------------------------------------------------------------
// Line data — simpler than OHLC, just time + value
// ---------------------------------------------------------------------------

/// A single data point for a line series
#[derive(Debug, Clone, Copy)]
pub struct LineDataPoint {
    pub time: i64,
    pub value: f64,
}

// ---------------------------------------------------------------------------
// Series options
// ---------------------------------------------------------------------------

/// Options specific to candlestick series
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct CandlestickOptions {
    pub up_color: Color,
    pub down_color: Color,
    pub border_up_color: Color,
    pub border_down_color: Color,
    pub wick_up_color: Color,
    pub wick_down_color: Color,
    /// Whether to draw hollow candles for bullish bars
    pub hollow: bool,
}

impl Default for CandlestickOptions {
    fn default() -> Self {
        CandlestickOptions {
            up_color: ColorName::Teal.color(),
            down_color: ColorName::Red.color(),
            border_up_color: ColorName::Teal.color(),
            border_down_color: ColorName::Red.color(),
            wick_up_color: ColorName::Teal.color(),
            wick_down_color: ColorName::Red.color(),
            hollow: false,
        }
    }
}

/// Options specific to line series
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct LineSeriesOptions {
    pub color: Color,
    pub line_width: f32,
    /// Whether to draw circles at each data point
    pub point_markers_visible: bool,
    pub point_markers_radius: f32,
}

impl Default for LineSeriesOptions {
    fn default() -> Self {
        LineSeriesOptions {
            color: ColorName::Blue.color(),
            line_width: 2.0,
            point_markers_visible: false,
            point_markers_radius: 3.0,
        }
    }
}

/// Options specific to area series (LWC: AreaSeriesOptions)
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct AreaSeriesOptions {
    /// Line color at the top
    pub line_color: Color,
    pub line_width: f32,
    /// Gradient top color (at the line)
    pub top_color: Color,
    /// Gradient bottom color (at the chart bottom)
    pub bottom_color: Color,
}

impl Default for AreaSeriesOptions {
    fn default() -> Self {
        AreaSeriesOptions {
            line_color: ColorName::Blue.color(),
            line_width: 2.0,
            top_color: ColorName::Blue.color().with_alpha(0.4),
            bottom_color: ColorName::Blue.color().with_alpha(0.0),
        }
    }
}

/// Options specific to histogram series (LWC: HistogramSeriesOptions)
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct HistogramSeriesOptions {
    /// Default bar color
    pub color: Color,
    /// Base value (usually 0.0) — bars extend from base to value
    pub base: f64,
}

impl Default for HistogramSeriesOptions {
    fn default() -> Self {
        HistogramSeriesOptions {
            color: ColorName::Blue.color().with_alpha(0.7),
            base: 0.0,
        }
    }
}

/// A single histogram data point with optional per-bar color
#[derive(Debug, Clone, Copy)]
pub struct HistogramDataPoint {
    pub time: i64,
    pub value: f64,
    /// Per-bar color override (if None, uses series default)
    pub color: Option<Color>,
}

/// Options specific to baseline series (LWC: BaselineSeriesOptions)
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct BaselineSeriesOptions {
    /// The baseline value that splits top/bottom areas
    pub base_value: f64,
    /// Color and fill for the region above the baseline
    pub top_line_color: Color,
    pub top_fill_color: Color,
    /// Color and fill for the region below the baseline
    pub bottom_line_color: Color,
    pub bottom_fill_color: Color,
    pub line_width: f32,
}

impl Default for BaselineSeriesOptions {
    fn default() -> Self {
        BaselineSeriesOptions {
            base_value: 0.0,
            top_line_color: ColorName::Teal.color(),
            top_fill_color: ColorName::Teal.color().with_alpha(0.2),
            bottom_line_color: ColorName::Red.color(),
            bottom_fill_color: ColorName::Red.color().with_alpha(0.2),
            line_width: 2.0,
        }
    }
}

// ---------------------------------------------------------------------------
// Series data — typed union of data kinds
// ---------------------------------------------------------------------------

/// Series data can be OHLC bars, line points, or histogram points
#[derive(Debug, Clone)]
pub enum SeriesData {
    Ohlc(Vec<OhlcBar>),
    Line(Vec<LineDataPoint>),
    Histogram(Vec<HistogramDataPoint>),
}

impl SeriesData {
    pub fn len(&self) -> usize {
        match self {
            SeriesData::Ohlc(bars) => bars.len(),
            SeriesData::Line(pts) => pts.len(),
            SeriesData::Histogram(pts) => pts.len(),
        }
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Get the close/value at a given index
    pub fn value_at(&self, index: usize) -> Option<f64> {
        match self {
            SeriesData::Ohlc(bars) => bars.get(index).map(|b| b.close),
            SeriesData::Line(pts) => pts.get(index).map(|p| p.value),
            SeriesData::Histogram(pts) => pts.get(index).map(|p| p.value),
        }
    }

    /// Get the time at a given index
    pub fn time_at(&self, index: usize) -> Option<i64> {
        match self {
            SeriesData::Ohlc(bars) => bars.get(index).map(|b| b.time),
            SeriesData::Line(pts) => pts.get(index).map(|p| p.time),
            SeriesData::Histogram(pts) => pts.get(index).map(|p| p.time),
        }
    }

    /// Get min/max values for price scale fitting
    pub fn min_max(&self) -> Option<(f64, f64)> {
        match self {
            SeriesData::Ohlc(bars) => {
                if bars.is_empty() {
                    return None;
                }
                let min = bars.iter().map(|b| b.low).fold(f64::INFINITY, f64::min);
                let max = bars
                    .iter()
                    .map(|b| b.high)
                    .fold(f64::NEG_INFINITY, f64::max);
                Some((min, max))
            }
            SeriesData::Line(pts) => {
                if pts.is_empty() {
                    return None;
                }
                let min = pts.iter().map(|p| p.value).fold(f64::INFINITY, f64::min);
                let max = pts
                    .iter()
                    .map(|p| p.value)
                    .fold(f64::NEG_INFINITY, f64::max);
                Some((min, max))
            }
            SeriesData::Histogram(pts) => {
                if pts.is_empty() {
                    return None;
                }
                let min = pts.iter().map(|p| p.value).fold(f64::INFINITY, f64::min);
                let max = pts
                    .iter()
                    .map(|p| p.value)
                    .fold(f64::NEG_INFINITY, f64::max);
                Some((min, max))
            }
        }
    }

    pub fn set_ohlc(&mut self, bars: Vec<OhlcBar>) {
        if let SeriesData::Ohlc(ref mut data) = self {
            *data = bars;
        }
    }

    pub fn set_line(&mut self, pts: Vec<LineDataPoint>) {
        if let SeriesData::Line(ref mut data) = self {
            *data = pts;
        }
    }

    pub fn set_histogram(&mut self, pts: Vec<HistogramDataPoint>) {
        if let SeriesData::Histogram(ref mut data) = self {
            *data = pts;
        }
    }

    pub fn update_ohlc(&mut self, bar: OhlcBar) {
        if let SeriesData::Ohlc(ref mut data) = self {
            if let Some(last) = data.last_mut() {
                if last.time == bar.time {
                    *last = bar;
                    return;
                } else if bar.time < last.time {
                    // For MVP historical updates, do binary search and replace if exists
                    if let Ok(idx) = data.binary_search_by_key(&bar.time, |b| b.time) {
                        data[idx] = bar;
                    }
                    return;
                }
            }
            data.push(bar);
        }
    }

    pub fn update_line(&mut self, pt: LineDataPoint) {
        if let SeriesData::Line(ref mut data) = self {
            if let Some(last) = data.last_mut() {
                if last.time == pt.time {
                    *last = pt;
                    return;
                } else if pt.time < last.time {
                    if let Ok(idx) = data.binary_search_by_key(&pt.time, |p| p.time) {
                        data[idx] = pt;
                    }
                    return;
                }
            }
            data.push(pt);
        }
    }

    pub fn update_histogram(&mut self, pt: HistogramDataPoint) {
        if let SeriesData::Histogram(ref mut data) = self {
            if let Some(last) = data.last_mut() {
                if last.time == pt.time {
                    *last = pt;
                    return;
                } else if pt.time < last.time {
                    if let Ok(idx) = data.binary_search_by_key(&pt.time, |p| p.time) {
                        data[idx] = pt;
                    }
                    return;
                }
            }
            data.push(pt);
        }
    }

    pub fn pop(&mut self, count: usize) {
        match self {
            SeriesData::Ohlc(bars) => {
                bars.truncate(bars.len().saturating_sub(count));
            }
            SeriesData::Line(pts) => {
                pts.truncate(pts.len().saturating_sub(count));
            }
            SeriesData::Histogram(pts) => {
                pts.truncate(pts.len().saturating_sub(count));
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Price Lines
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct PriceLineOptions {
    pub price: f64,
    pub color: Color,
    pub line_width: f32,
    /// 0=Solid, 1=Dotted, 2=Dashed
    pub line_style: u8,
    pub title: String,
}

impl Default for PriceLineOptions {
    fn default() -> Self {
        PriceLineOptions {
            price: 0.0,
            color: ColorName::Crimson.color(),
            line_width: 1.0,
            line_style: 0,
            title: String::new(),
        }
    }
}

// ---------------------------------------------------------------------------
// Series: a single series on the chart
// ---------------------------------------------------------------------------

/// A chart series with typed data and options
#[derive(Debug, Clone)]
pub struct Series {
    pub id: u32,
    pub series_type: SeriesType,
    pub data: SeriesData,
    pub candlestick_options: CandlestickOptions,
    pub line_options: LineSeriesOptions,
    pub area_options: AreaSeriesOptions,
    pub histogram_options: HistogramSeriesOptions,
    pub baseline_options: BaselineSeriesOptions,
    pub visible: bool,
    pub pane_index: usize,
    pub price_lines: Vec<(u32, PriceLineOptions)>,
    pub next_price_line_id: u32,
}

impl Series {
    /// Create a new OHLC series
    pub fn ohlc(id: u32, bars: Vec<OhlcBar>) -> Self {
        Series {
            id,
            series_type: SeriesType::Ohlc,
            data: SeriesData::Ohlc(bars),
            candlestick_options: CandlestickOptions::default(),
            line_options: LineSeriesOptions::default(),
            area_options: AreaSeriesOptions::default(),
            histogram_options: HistogramSeriesOptions::default(),
            baseline_options: BaselineSeriesOptions::default(),
            visible: true,
            pane_index: 0,
            price_lines: Vec::new(),
            next_price_line_id: 1,
        }
    }

    /// Create a new candlestick series
    pub fn candlestick(id: u32, bars: Vec<OhlcBar>) -> Self {
        Series {
            id,
            series_type: SeriesType::Candlestick,
            data: SeriesData::Ohlc(bars),
            candlestick_options: CandlestickOptions::default(),
            line_options: LineSeriesOptions::default(),
            area_options: AreaSeriesOptions::default(),
            histogram_options: HistogramSeriesOptions::default(),
            baseline_options: BaselineSeriesOptions::default(),
            visible: true,
            pane_index: 0,
            price_lines: Vec::new(),
            next_price_line_id: 1,
        }
    }

    /// Create a new line series
    pub fn line(id: u32, points: Vec<LineDataPoint>) -> Self {
        Series {
            id,
            series_type: SeriesType::Line,
            data: SeriesData::Line(points),
            candlestick_options: CandlestickOptions::default(),
            line_options: LineSeriesOptions::default(),
            area_options: AreaSeriesOptions::default(),
            histogram_options: HistogramSeriesOptions::default(),
            baseline_options: BaselineSeriesOptions::default(),
            visible: true,
            pane_index: 0,
            price_lines: Vec::new(),
            next_price_line_id: 1,
        }
    }

    /// Create a new area series (line + gradient fill)
    pub fn area(id: u32, points: Vec<LineDataPoint>) -> Self {
        Series {
            id,
            series_type: SeriesType::Area,
            data: SeriesData::Line(points),
            candlestick_options: CandlestickOptions::default(),
            line_options: LineSeriesOptions::default(),
            area_options: AreaSeriesOptions::default(),
            histogram_options: HistogramSeriesOptions::default(),
            baseline_options: BaselineSeriesOptions::default(),
            visible: true,
            pane_index: 0,
            price_lines: Vec::new(),
            next_price_line_id: 1,
        }
    }

    /// Create a new histogram series
    pub fn histogram(id: u32, points: Vec<HistogramDataPoint>) -> Self {
        Series {
            id,
            series_type: SeriesType::Histogram,
            data: SeriesData::Histogram(points),
            candlestick_options: CandlestickOptions::default(),
            line_options: LineSeriesOptions::default(),
            area_options: AreaSeriesOptions::default(),
            histogram_options: HistogramSeriesOptions::default(),
            baseline_options: BaselineSeriesOptions::default(),
            visible: true,
            pane_index: 0,
            price_lines: Vec::new(),
            next_price_line_id: 1,
        }
    }

    /// Create a new baseline series (line with top/bottom fills)
    pub fn baseline(id: u32, points: Vec<LineDataPoint>, base_value: f64) -> Self {
        let mut s = Series {
            id,
            series_type: SeriesType::Baseline,
            data: SeriesData::Line(points),
            candlestick_options: CandlestickOptions::default(),
            line_options: LineSeriesOptions::default(),
            area_options: AreaSeriesOptions::default(),
            histogram_options: HistogramSeriesOptions::default(),
            baseline_options: BaselineSeriesOptions::default(),
            visible: true,
            pane_index: 0,
            price_lines: Vec::new(),
            next_price_line_id: 1,
        };
        s.baseline_options.base_value = base_value;
        s
    }

    /// Add a generic price line to this series
    pub fn add_price_line(&mut self, options: PriceLineOptions) -> u32 {
        let id = self.next_price_line_id;
        self.next_price_line_id += 1;
        self.price_lines.push((id, options));
        id
    }

    /// Remove a price line by its internally generated ID
    pub fn remove_price_line(&mut self, id: u32) -> bool {
        let before = self.price_lines.len();
        self.price_lines.retain(|(lid, _)| *lid != id);
        self.price_lines.len() < before
    }

    /// Is this series bullish at a given bar?
    pub fn is_bullish_at(&self, index: usize) -> bool {
        match &self.data {
            SeriesData::Ohlc(bars) => bars.get(index).map_or(false, |b| b.close >= b.open),
            SeriesData::Line(_) | SeriesData::Histogram(_) => true,
        }
    }

    /// Applies a JSON string of partial options to the current series options.
    pub fn apply_options_json(&mut self, json_str: &str) -> bool {
        let partial = match serde_json::from_str::<serde_json::Value>(json_str) {
            Ok(v) => v,
            Err(_) => return false,
        };

        match self.series_type {
            SeriesType::Ohlc | SeriesType::Candlestick => {
                if let Ok(mut full) = serde_json::to_value(&self.candlestick_options) {
                    crate::chart_options::merge_json(&mut full, partial);
                    if let Ok(new_opts) = serde_json::from_value(full) {
                        self.candlestick_options = new_opts;
                        return true;
                    }
                }
            }
            SeriesType::Line => {
                if let Ok(mut full) = serde_json::to_value(&self.line_options) {
                    crate::chart_options::merge_json(&mut full, partial);
                    if let Ok(new_opts) = serde_json::from_value(full) {
                        self.line_options = new_opts;
                        return true;
                    }
                }
            }
            SeriesType::Area => {
                if let Ok(mut full) = serde_json::to_value(&self.area_options) {
                    crate::chart_options::merge_json(&mut full, partial);
                    if let Ok(new_opts) = serde_json::from_value(full) {
                        self.area_options = new_opts;
                        return true;
                    }
                }
            }
            SeriesType::Histogram => {
                if let Ok(mut full) = serde_json::to_value(&self.histogram_options) {
                    crate::chart_options::merge_json(&mut full, partial);
                    if let Ok(new_opts) = serde_json::from_value(full) {
                        self.histogram_options = new_opts;
                        return true;
                    }
                }
            }
            SeriesType::Baseline => {
                if let Ok(mut full) = serde_json::to_value(&self.baseline_options) {
                    crate::chart_options::merge_json(&mut full, partial);
                    if let Ok(new_opts) = serde_json::from_value(full) {
                        self.baseline_options = new_opts;
                        return true;
                    }
                }
            }
        }
        false
    }
}

// ---------------------------------------------------------------------------
// SeriesCollection: multi-series management
// ---------------------------------------------------------------------------

/// Manages multiple series on a chart
#[derive(Debug, Clone, Default)]
pub struct SeriesCollection {
    pub series: Vec<Series>,
    next_id: u32,
}

impl SeriesCollection {
    pub fn new() -> Self {
        Self::default()
    }

    /// Add a series, returning its assigned ID.
    pub fn add(&mut self, mut series: Series) -> u32 {
        let id = self.next_id;
        self.next_id += 1;
        series.id = id;
        self.series.push(series);
        id
    }

    /// Remove a series by ID. Returns true if found.
    pub fn remove(&mut self, id: u32) -> bool {
        let before = self.series.len();
        self.series.retain(|s| s.id != id);
        self.series.len() < before
    }

    /// Get a series by ID.
    pub fn get(&self, id: u32) -> Option<&Series> {
        self.series.iter().find(|s| s.id == id)
    }

    /// Get a mutable reference to a series by ID.
    pub fn get_mut(&mut self, id: u32) -> Option<&mut Series> {
        self.series.iter_mut().find(|s| s.id == id)
    }

    /// Number of series.
    pub fn len(&self) -> usize {
        self.series.len()
    }

    pub fn is_empty(&self) -> bool {
        self.series.is_empty()
    }
}

// ---------------------------------------------------------------------------
// Tests — TDD
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn make_bar(time: i64, open: f64, close: f64) -> OhlcBar {
        OhlcBar {
            time,
            open,
            high: open.max(close) + 1.0,
            low: open.min(close) - 1.0,
            close,
        }
    }

    fn make_line_pt(time: i64, value: f64) -> LineDataPoint {
        LineDataPoint { time, value }
    }

    // --- Series type tests ---

    #[test]
    fn test_ohlc_series_create() {
        let bars = vec![make_bar(1, 100.0, 105.0)];
        let s = Series::ohlc(0, bars);
        assert_eq!(s.series_type, SeriesType::Ohlc);
        assert!(s.visible);
        assert_eq!(s.data.len(), 1);
    }

    #[test]
    fn test_candlestick_series_create() {
        let bars = vec![make_bar(1, 100.0, 105.0), make_bar(2, 105.0, 98.0)];
        let s = Series::candlestick(0, bars);
        assert_eq!(s.series_type, SeriesType::Candlestick);
        assert_eq!(s.data.len(), 2);
    }

    #[test]
    fn test_line_series_create() {
        let pts = vec![make_line_pt(1, 50.0), make_line_pt(2, 55.0)];
        let s = Series::line(0, pts);
        assert_eq!(s.series_type, SeriesType::Line);
        assert_eq!(s.data.len(), 2);
    }

    #[test]
    fn test_is_bullish() {
        let bars = vec![make_bar(1, 100.0, 105.0), make_bar(2, 105.0, 98.0)];
        let s = Series::candlestick(0, bars);
        assert!(s.is_bullish_at(0)); // close > open
        assert!(!s.is_bullish_at(1)); // close < open
    }

    // --- SeriesData tests ---

    #[test]
    fn test_series_data_value_at() {
        let data = SeriesData::Ohlc(vec![make_bar(1, 100.0, 105.0)]);
        assert!((data.value_at(0).unwrap() - 105.0).abs() < f64::EPSILON);
        assert!(data.value_at(1).is_none());

        let line_data = SeriesData::Line(vec![make_line_pt(1, 42.0)]);
        assert!((line_data.value_at(0).unwrap() - 42.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_series_data_time_at() {
        let data = SeriesData::Ohlc(vec![make_bar(1000, 100.0, 105.0)]);
        assert_eq!(data.time_at(0), Some(1000));

        let line_data = SeriesData::Line(vec![make_line_pt(2000, 42.0)]);
        assert_eq!(line_data.time_at(0), Some(2000));
    }

    #[test]
    fn test_series_data_min_max_ohlc() {
        let data = SeriesData::Ohlc(vec![
            make_bar(1, 100.0, 105.0), // low=99, high=106
            make_bar(2, 90.0, 95.0),   // low=89, high=96
        ]);
        let (min, max) = data.min_max().unwrap();
        assert!((min - 89.0).abs() < f64::EPSILON);
        assert!((max - 106.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_series_data_min_max_line() {
        let data = SeriesData::Line(vec![make_line_pt(1, 10.0), make_line_pt(2, 30.0)]);
        let (min, max) = data.min_max().unwrap();
        assert!((min - 10.0).abs() < f64::EPSILON);
        assert!((max - 30.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_series_data_min_max_empty() {
        let data = SeriesData::Ohlc(vec![]);
        assert!(data.min_max().is_none());
    }

    // --- SeriesCollection tests ---

    #[test]
    fn test_collection_add_remove() {
        let mut coll = SeriesCollection::new();
        let id1 = coll.add(Series::ohlc(0, vec![make_bar(1, 100.0, 105.0)]));
        let id2 = coll.add(Series::line(0, vec![make_line_pt(1, 50.0)]));

        assert_eq!(coll.len(), 2);
        assert!(coll.get(id1).is_some());
        assert!(coll.get(id2).is_some());

        assert!(coll.remove(id1));
        assert_eq!(coll.len(), 1);
        assert!(coll.get(id1).is_none());
        assert!(coll.get(id2).is_some());
    }

    #[test]
    fn test_collection_remove_nonexistent() {
        let mut coll = SeriesCollection::new();
        assert!(!coll.remove(999));
    }

    #[test]
    fn test_collection_get_mut() {
        let mut coll = SeriesCollection::new();
        let id = coll.add(Series::line(0, vec![make_line_pt(1, 50.0)]));

        let s = coll.get_mut(id).unwrap();
        s.visible = false;
        assert!(!coll.get(id).unwrap().visible);
    }

    #[test]
    fn test_collection_ids_auto_increment() {
        let mut coll = SeriesCollection::new();
        let id1 = coll.add(Series::ohlc(0, vec![]));
        let id2 = coll.add(Series::ohlc(0, vec![]));
        let id3 = coll.add(Series::ohlc(0, vec![]));
        assert_eq!(id1, 0);
        assert_eq!(id2, 1);
        assert_eq!(id3, 2);
    }

    #[test]
    fn test_candlestick_options_default() {
        let opts = CandlestickOptions::default();
        assert!(!opts.hollow);
        assert_eq!(opts.up_color, ColorName::Teal.color());
    }

    #[test]
    fn test_line_options_default() {
        let opts = LineSeriesOptions::default();
        assert_eq!(opts.line_width, 2.0);
        assert!(!opts.point_markers_visible);
    }

    #[test]
    fn test_series_data_is_empty() {
        assert!(SeriesData::Ohlc(vec![]).is_empty());
        assert!(!SeriesData::Line(vec![make_line_pt(1, 1.0)]).is_empty());
    }

    // --- New series type tests (Slice 6) ---

    #[test]
    fn test_area_series_create() {
        let pts = vec![make_line_pt(1, 50.0), make_line_pt(2, 55.0)];
        let s = Series::area(0, pts);
        assert_eq!(s.series_type, SeriesType::Area);
        assert_eq!(s.data.len(), 2);
        // Area options should have defaults
        assert_eq!(s.area_options.line_width, 2.0);
    }

    #[test]
    fn test_histogram_series_create() {
        let pts = vec![
            HistogramDataPoint {
                time: 1,
                value: 100.0,
                color: None,
            },
            HistogramDataPoint {
                time: 2,
                value: -50.0,
                color: Some(Color::rgba(1.0, 0.0, 0.0, 1.0)),
            },
        ];
        let s = Series::histogram(0, pts);
        assert_eq!(s.series_type, SeriesType::Histogram);
        assert_eq!(s.data.len(), 2);
    }

    #[test]
    fn test_baseline_series_create() {
        let pts = vec![make_line_pt(1, 50.0)];
        let s = Series::baseline(0, pts, 42.0);
        assert_eq!(s.series_type, SeriesType::Baseline);
        assert_eq!(s.baseline_options.base_value, 42.0);
    }

    #[test]
    fn test_histogram_data_min_max() {
        let data = SeriesData::Histogram(vec![
            HistogramDataPoint {
                time: 1,
                value: 10.0,
                color: None,
            },
            HistogramDataPoint {
                time: 2,
                value: -5.0,
                color: None,
            },
            HistogramDataPoint {
                time: 3,
                value: 30.0,
                color: None,
            },
        ]);
        let (min, max) = data.min_max().unwrap();
        assert!((min - (-5.0)).abs() < f64::EPSILON);
        assert!((max - 30.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_histogram_data_value_at() {
        let data = SeriesData::Histogram(vec![HistogramDataPoint {
            time: 1,
            value: 42.0,
            color: None,
        }]);
        assert_eq!(data.value_at(0), Some(42.0));
        assert_eq!(data.value_at(1), None);
    }

    #[test]
    fn test_area_options_default() {
        let opts = AreaSeriesOptions::default();
        assert_eq!(opts.line_width, 2.0);
    }

    #[test]
    fn test_histogram_options_default() {
        let opts = HistogramSeriesOptions::default();
        assert_eq!(opts.base, 0.0);
    }

    #[test]
    fn test_baseline_options_default() {
        let opts = BaselineSeriesOptions::default();
        assert_eq!(opts.base_value, 0.0);
        assert_eq!(opts.line_width, 2.0);
    }
}
