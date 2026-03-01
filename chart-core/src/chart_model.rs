/// OHLC bar data point
#[derive(Debug, Clone, Copy)]
pub struct OhlcBar {
    pub time: i64, // Unix timestamp (seconds)
    pub open: f64,
    pub high: f64,
    pub low: f64,
    pub close: f64,
}

/// All chart data
pub struct ChartData {
    pub bars: Vec<OhlcBar>,
}

/// Edge margins around the plot area
#[derive(Debug, Clone, Copy)]
pub struct Margins {
    pub top: f32,
    pub right: f32,  // space for Y-axis labels
    pub bottom: f32, // space for X-axis labels
    pub left: f32,
}

impl Default for Margins {
    fn default() -> Self {
        Margins {
            top: 20.0,
            right: 80.0,
            bottom: 35.0,
            left: 10.0,
        }
    }
}

/// Computed rectangle
#[derive(Debug, Clone, Copy)]
pub struct Rect {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
}

/// Overall layout for the chart
pub struct ChartLayout {
    pub width: f32,
    pub height: f32,
    pub scale_factor: f64,
    pub margins: Margins,
    pub plot_area: Rect,
}

impl ChartLayout {
    pub fn new(width: f32, height: f32, scale_factor: f64) -> Self {
        let margins = Margins::default();
        let plot_area = Rect {
            x: margins.left,
            y: margins.top,
            width: (width - margins.left - margins.right).max(1.0),
            height: (height - margins.top - margins.bottom).max(1.0),
        };
        ChartLayout {
            width,
            height,
            scale_factor,
            margins,
            plot_area,
        }
    }

    /// Test if a point (logical coords) is inside the plot area
    pub fn plot_area_contains(&self, x: f32, y: f32) -> bool {
        x >= self.plot_area.x
            && x <= self.plot_area.x + self.plot_area.width
            && y >= self.plot_area.y
            && y <= self.plot_area.y + self.plot_area.height
    }

    /// Test if a point is in the Y-axis (price scale) area (right margin)
    pub fn y_axis_contains(&self, x: f32, y: f32) -> bool {
        x > self.plot_area.x + self.plot_area.width
            && x <= self.width
            && y >= self.plot_area.y
            && y <= self.plot_area.y + self.plot_area.height
    }

    /// Test if a point is in the X-axis (time scale) area (bottom margin)
    pub fn x_axis_contains(&self, x: f32, y: f32) -> bool {
        x >= self.plot_area.x
            && x <= self.plot_area.x + self.plot_area.width
            && y > self.plot_area.y + self.plot_area.height
            && y <= self.height
    }
}
