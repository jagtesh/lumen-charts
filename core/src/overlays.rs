use crate::chart_model::OhlcBar;
use crate::draw_backend::{Color, ColorName};

// ---------------------------------------------------------------------------
// Price lines: horizontal lines drawn at a specific price level
// ---------------------------------------------------------------------------

/// Style for a price line
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum LineStyle {
    Solid,
    Dashed,
    Dotted,
    LargeDashed,
    SparseDotted,
}

impl Default for LineStyle {
    fn default() -> Self {
        LineStyle::Solid
    }
}

/// A horizontal price line drawn across the chart at a specific price.
#[derive(Debug, Clone)]
pub struct PriceLine {
    pub id: u32,
    pub price: f64,
    pub color: Color,
    pub line_width: f32,
    pub line_style: LineStyle,
    pub label: String,
    pub label_visible: bool,
}

impl PriceLine {
    pub fn new(id: u32, price: f64) -> Self {
        PriceLine {
            id,
            price,
            color: ColorName::SlateGray.color().with_alpha(0.8),
            line_width: 1.0,
            line_style: LineStyle::Dashed,
            label: format!("{:.2}", price),
            label_visible: true,
        }
    }

    pub fn with_color(mut self, color: Color) -> Self {
        self.color = color;
        self
    }

    pub fn with_label(mut self, label: impl Into<String>) -> Self {
        self.label = label.into();
        self
    }

    pub fn with_style(mut self, style: LineStyle) -> Self {
        self.line_style = style;
        self
    }
}

// ---------------------------------------------------------------------------
// Series markers: symbols placed at specific data points
// ---------------------------------------------------------------------------

/// Shape of a marker
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum MarkerShape {
    ArrowUp,
    ArrowDown,
    Circle,
    Square,
}

/// Position relative to the bar
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum MarkerPosition {
    AboveBar,
    BelowBar,
    AtPrice,
}

/// A marker placed on a specific bar
#[derive(Debug, Clone)]
pub struct SeriesMarker {
    pub time: i64,
    pub shape: MarkerShape,
    pub position: MarkerPosition,
    pub color: Color,
    pub size: f32,
    pub text: String,
}

impl SeriesMarker {
    pub fn new(time: i64, shape: MarkerShape, position: MarkerPosition) -> Self {
        SeriesMarker {
            time,
            shape,
            position,
            color: ColorName::Teal.color(),
            size: 8.0,
            text: String::new(),
        }
    }

    pub fn with_color(mut self, color: Color) -> Self {
        self.color = color;
        self
    }

    pub fn with_text(mut self, text: impl Into<String>) -> Self {
        self.text = text.into();
        self
    }

    pub fn with_size(mut self, size: f32) -> Self {
        self.size = size;
        self
    }

    /// Compute the Y coordinate for this marker given the bar data.
    pub fn y_price(&self, bar: &OhlcBar) -> f64 {
        match self.position {
            MarkerPosition::AboveBar => bar.high,
            MarkerPosition::BelowBar => bar.low,
            MarkerPosition::AtPrice => bar.close,
        }
    }
}

// ---------------------------------------------------------------------------
// Last value marker: colored label on the Y-axis showing current price
// ---------------------------------------------------------------------------

/// Style for the last-value price label on the Y-axis
#[derive(Debug, Clone)]
pub struct LastValueMarker {
    pub visible: bool,
    pub color: Color,
}

impl Default for LastValueMarker {
    fn default() -> Self {
        LastValueMarker {
            visible: true,
            color: ColorName::Teal.color(),
        }
    }
}

// ---------------------------------------------------------------------------
// Watermark: text drawn in the background of the chart
// ---------------------------------------------------------------------------

/// Horizontal alignment
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum HAlign {
    Left,
    Center,
    Right,
}

/// Vertical alignment
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum VAlign {
    Top,
    Center,
    Bottom,
}

/// A watermark displayed in the chart background
#[derive(Debug, Clone)]
pub struct Watermark {
    pub text: String,
    pub font_size: f32,
    pub color: Color,
    pub h_align: HAlign,
    pub v_align: VAlign,
    pub visible: bool,
}

impl Default for Watermark {
    fn default() -> Self {
        Watermark {
            text: String::new(),
            font_size: 48.0,
            color: ColorName::DarkOlive.color().with_alpha(0.3),
            h_align: HAlign::Center,
            v_align: VAlign::Center,
            visible: false,
        }
    }
}

impl Watermark {
    pub fn new(text: impl Into<String>) -> Self {
        Watermark {
            text: text.into(),
            visible: true,
            ..Default::default()
        }
    }
}

// ---------------------------------------------------------------------------
// Overlay collection: owned by ChartState
// ---------------------------------------------------------------------------

/// All overlay elements for a chart
#[derive(Debug, Clone, Default)]
pub struct Overlays {
    pub price_lines: Vec<PriceLine>,
    pub markers: Vec<SeriesMarker>,
    pub last_value: LastValueMarker,
    pub watermark: Watermark,
    next_price_line_id: u32,
}

impl Overlays {
    pub fn new() -> Self {
        Self::default()
    }

    // --- Price lines ---

    /// Add a price line, returning its ID.
    pub fn add_price_line(&mut self, price: f64) -> u32 {
        let id = self.next_price_line_id;
        self.next_price_line_id += 1;
        self.price_lines.push(PriceLine::new(id, price));
        id
    }

    /// Add a fully configured price line.
    pub fn add_price_line_with(&mut self, line: PriceLine) -> u32 {
        let id = line.id;
        self.price_lines.push(line);
        if id >= self.next_price_line_id {
            self.next_price_line_id = id + 1;
        }
        id
    }

    /// Remove a price line by ID.
    pub fn remove_price_line(&mut self, id: u32) -> bool {
        let before = self.price_lines.len();
        self.price_lines.retain(|l| l.id != id);
        self.price_lines.len() < before
    }

    /// Get a price line by ID.
    pub fn get_price_line(&self, id: u32) -> Option<&PriceLine> {
        self.price_lines.iter().find(|l| l.id == id)
    }

    // --- Markers ---

    /// Set markers (replaces all existing markers).
    pub fn set_markers(&mut self, markers: Vec<SeriesMarker>) {
        self.markers = markers;
    }

    /// Add a single marker.
    pub fn add_marker(&mut self, marker: SeriesMarker) {
        self.markers.push(marker);
    }

    /// Clear all markers.
    pub fn clear_markers(&mut self) {
        self.markers.clear();
    }

    // --- Watermark ---

    /// Set watermark text (makes it visible).
    pub fn set_watermark(&mut self, text: impl Into<String>) {
        self.watermark = Watermark::new(text);
    }

    /// Hide the watermark.
    pub fn hide_watermark(&mut self) {
        self.watermark.visible = false;
    }

    // --- Markers JSON (moved from C-ABI wrapper) ---

    /// Set markers from a JSON array string.
    /// JSON format: [{"time":1704153600,"shape":"arrowUp","position":"belowBar",
    ///   "color":[0.15,0.65,0.6,1.0],"size":8,"text":"Buy"}, ...]
    pub fn set_markers_from_json(&mut self, json_str: &str) -> bool {
        let items: Vec<serde_json::Value> = match serde_json::from_str(json_str) {
            Ok(v) => v,
            Err(_) => return false,
        };

        let mut markers = Vec::with_capacity(items.len());
        for item in &items {
            let time = item.get("time").and_then(|v| v.as_i64()).unwrap_or(0);
            let shape_str = item
                .get("shape")
                .and_then(|v| v.as_str())
                .unwrap_or("circle");
            let pos_str = item
                .get("position")
                .and_then(|v| v.as_str())
                .unwrap_or("aboveBar");

            let shape = match shape_str {
                "arrowUp" => MarkerShape::ArrowUp,
                "arrowDown" => MarkerShape::ArrowDown,
                "square" => MarkerShape::Square,
                _ => MarkerShape::Circle,
            };
            let position = match pos_str {
                "belowBar" => MarkerPosition::BelowBar,
                "atPrice" => MarkerPosition::AtPrice,
                _ => MarkerPosition::AboveBar,
            };

            let mut marker = SeriesMarker::new(time, shape, position);

            if let Some(color) = item.get("color").and_then(|v| v.as_array()) {
                if color.len() == 4 {
                    marker.color = Color([
                        color[0].as_f64().unwrap_or(0.0) as f32,
                        color[1].as_f64().unwrap_or(0.0) as f32,
                        color[2].as_f64().unwrap_or(0.0) as f32,
                        color[3].as_f64().unwrap_or(1.0) as f32,
                    ]);
                }
            }
            if let Some(size) = item.get("size").and_then(|v| v.as_f64()) {
                marker.size = size as f32;
            }
            if let Some(text) = item.get("text").and_then(|v| v.as_str()) {
                marker.text = text.to_string();
            }

            markers.push(marker);
        }

        self.markers = markers;
        true
    }

    /// Serialize markers to a JSON string.
    pub fn markers_to_json(&self) -> String {
        let mut arr = Vec::new();
        for m in &self.markers {
            let shape_str = match m.shape {
                MarkerShape::ArrowUp => "arrowUp",
                MarkerShape::ArrowDown => "arrowDown",
                MarkerShape::Circle => "circle",
                MarkerShape::Square => "square",
            };
            let pos_str = match m.position {
                MarkerPosition::AboveBar => "aboveBar",
                MarkerPosition::BelowBar => "belowBar",
                MarkerPosition::AtPrice => "atPrice",
            };
            arr.push(serde_json::json!({
                "time": m.time,
                "shape": shape_str,
                "position": pos_str,
                "color": m.color,
                "size": m.size,
                "text": m.text,
            }));
        }
        serde_json::to_string(&arr).unwrap_or_else(|_| "[]".to_string())
    }
}

// ---------------------------------------------------------------------------
// Tests — TDD: written before rendering integration
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_price_line_create() {
        let pl = PriceLine::new(0, 150.0);
        assert_eq!(pl.id, 0);
        assert!((pl.price - 150.0).abs() < f64::EPSILON);
        assert!(pl.label_visible);
        assert_eq!(pl.line_style, LineStyle::Dashed);
    }

    #[test]
    fn test_price_line_builder() {
        let pl = PriceLine::new(1, 200.0)
            .with_color(Color::rgba(1.0, 0.0, 0.0, 1.0))
            .with_label("Support")
            .with_style(LineStyle::Solid);
        assert_eq!(pl.label, "Support");
        assert_eq!(pl.line_style, LineStyle::Solid);
        assert_eq!(pl.color, Color::rgba(1.0, 0.0, 0.0, 1.0));
    }

    #[test]
    fn test_overlays_add_remove_price_line() {
        let mut ov = Overlays::new();
        let id1 = ov.add_price_line(100.0);
        let id2 = ov.add_price_line(200.0);

        assert_eq!(ov.price_lines.len(), 2);
        assert!(ov.get_price_line(id1).is_some());
        assert!(ov.get_price_line(id2).is_some());

        assert!(ov.remove_price_line(id1));
        assert_eq!(ov.price_lines.len(), 1);
        assert!(ov.get_price_line(id1).is_none());
        assert!(ov.get_price_line(id2).is_some());
    }

    #[test]
    fn test_remove_nonexistent_price_line() {
        let mut ov = Overlays::new();
        assert!(!ov.remove_price_line(999));
    }

    #[test]
    fn test_series_marker_create() {
        let marker = SeriesMarker::new(1000, MarkerShape::ArrowUp, MarkerPosition::BelowBar);
        assert_eq!(marker.time, 1000);
        assert_eq!(marker.shape, MarkerShape::ArrowUp);
        assert_eq!(marker.position, MarkerPosition::BelowBar);
        assert_eq!(marker.size, 8.0);
    }

    #[test]
    fn test_marker_y_price() {
        let bar = OhlcBar {
            time: 100,
            open: 50.0,
            high: 60.0,
            low: 40.0,
            close: 55.0,
        };

        let above = SeriesMarker::new(100, MarkerShape::ArrowDown, MarkerPosition::AboveBar);
        assert!((above.y_price(&bar) - 60.0).abs() < f64::EPSILON);

        let below = SeriesMarker::new(100, MarkerShape::ArrowUp, MarkerPosition::BelowBar);
        assert!((below.y_price(&bar) - 40.0).abs() < f64::EPSILON);

        let at_price = SeriesMarker::new(100, MarkerShape::Circle, MarkerPosition::AtPrice);
        assert!((at_price.y_price(&bar) - 55.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_marker_builder() {
        let marker = SeriesMarker::new(500, MarkerShape::Circle, MarkerPosition::AboveBar)
            .with_color(Color::rgba(1.0, 0.0, 0.0, 1.0))
            .with_text("Buy")
            .with_size(12.0);
        assert_eq!(marker.text, "Buy");
        assert_eq!(marker.size, 12.0);
    }

    #[test]
    fn test_overlays_markers() {
        let mut ov = Overlays::new();
        ov.add_marker(SeriesMarker::new(
            100,
            MarkerShape::ArrowUp,
            MarkerPosition::BelowBar,
        ));
        ov.add_marker(SeriesMarker::new(
            200,
            MarkerShape::ArrowDown,
            MarkerPosition::AboveBar,
        ));
        assert_eq!(ov.markers.len(), 2);

        ov.clear_markers();
        assert!(ov.markers.is_empty());
    }

    #[test]
    fn test_set_markers_replaces() {
        let mut ov = Overlays::new();
        ov.add_marker(SeriesMarker::new(
            100,
            MarkerShape::Circle,
            MarkerPosition::AtPrice,
        ));
        assert_eq!(ov.markers.len(), 1);

        ov.set_markers(vec![
            SeriesMarker::new(200, MarkerShape::Square, MarkerPosition::AboveBar),
            SeriesMarker::new(300, MarkerShape::Square, MarkerPosition::AboveBar),
        ]);
        assert_eq!(ov.markers.len(), 2);
        assert_eq!(ov.markers[0].time, 200);
    }

    #[test]
    fn test_watermark_default_hidden() {
        let wm = Watermark::default();
        assert!(!wm.visible);
        assert!(wm.text.is_empty());
    }

    #[test]
    fn test_watermark_new_visible() {
        let wm = Watermark::new("AAPL");
        assert!(wm.visible);
        assert_eq!(wm.text, "AAPL");
        assert_eq!(wm.font_size, 48.0);
    }

    #[test]
    fn test_overlays_watermark() {
        let mut ov = Overlays::new();
        assert!(!ov.watermark.visible);

        ov.set_watermark("BTC/USD");
        assert!(ov.watermark.visible);
        assert_eq!(ov.watermark.text, "BTC/USD");

        ov.hide_watermark();
        assert!(!ov.watermark.visible);
    }

    #[test]
    fn test_last_value_marker_default() {
        let lv = LastValueMarker::default();
        assert!(lv.visible);
    }

    #[test]
    fn test_price_line_ids_auto_increment() {
        let mut ov = Overlays::new();
        let id1 = ov.add_price_line(100.0);
        let id2 = ov.add_price_line(200.0);
        let id3 = ov.add_price_line(300.0);
        assert_eq!(id1, 0);
        assert_eq!(id2, 1);
        assert_eq!(id3, 2);
    }
}
