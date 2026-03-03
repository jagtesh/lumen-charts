/// DrawBackend trait — abstract rendering primitives for chart drawing.
///
/// All drawing functions in chart_renderer.rs are generic over `impl DrawBackend`,
/// enabling multiple rendering backends: Vello (WebGPU/Metal), Canvas 2D, WebGL (femtovg).
///
/// Colors use the `Color` newtype (RGBA f32, 0.0–1.0). Coordinates are f64 in logical space.
use serde::{Deserialize, Serialize};

// ---------------------------------------------------------------------------
// Color newtype — single canonical RGBA representation
// ---------------------------------------------------------------------------

/// A color in RGBA format, each component 0.0–1.0.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Color(pub [f32; 4]);

impl Color {
    /// Construct a color from RGBA components.
    pub const fn rgba(r: f32, g: f32, b: f32, a: f32) -> Self {
        Color([r, g, b, a])
    }

    /// Return a copy of this color with a different alpha value.
    pub const fn with_alpha(self, a: f32) -> Self {
        Color([self.0[0], self.0[1], self.0[2], a])
    }
}

impl From<[f32; 4]> for Color {
    fn from(c: [f32; 4]) -> Self {
        Color(c)
    }
}

impl From<Color> for [f32; 4] {
    fn from(c: Color) -> Self {
        c.0
    }
}

impl std::ops::Index<usize> for Color {
    type Output = f32;
    fn index(&self, i: usize) -> &f32 {
        &self.0[i]
    }
}

// ---------------------------------------------------------------------------
// Palette — semantic chart color roles
// ---------------------------------------------------------------------------

/// Named color palette for standard chart elements.
///
/// Each variant maps a *purpose* (e.g. background, grid) to a concrete `Color`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Palette {
    /// Main chart background
    Background,
    /// Grid lines
    Grid,
    /// Axis lines and tick marks
    Axis,
    /// Bullish (up) elements
    Bull,
    /// Bearish (down) elements
    Bear,
    /// General text / labels
    Text,
    /// Crosshair lines
    Crosshair,
    /// Crosshair price-label background
    CrosshairLabelBg,
    /// Crosshair OHLC-info background
    CrosshairInfoBg,
    /// Pure white (for text on colored backgrounds)
    White,
}

impl Palette {
    /// Resolve this palette entry to a concrete `Color`.
    pub const fn color(&self) -> Color {
        match self {
            Self::Background => Color::rgba(0.07, 0.07, 0.10, 1.0),
            Self::Grid => Color::rgba(0.15, 0.15, 0.20, 1.0),
            Self::Axis => Color::rgba(0.4, 0.4, 0.5, 1.0),
            Self::Bull => Color::rgba(0.15, 0.65, 0.60, 1.0),
            Self::Bear => Color::rgba(0.94, 0.33, 0.31, 1.0),
            Self::Text => Color::rgba(0.6, 0.6, 0.7, 1.0),
            Self::Crosshair => Color::rgba(0.5, 0.5, 0.6, 0.8),
            Self::CrosshairLabelBg => Color::rgba(0.2, 0.2, 0.3, 0.9),
            Self::CrosshairInfoBg => Color::rgba(0.12, 0.12, 0.18, 0.9),
            Self::White => Color::rgba(1.0, 1.0, 1.0, 1.0),
        }
    }
}

/// A gradient stop: (color, offset). Offset is 0.0 (start) to 1.0 (end).
pub type GradientStop = (Color, f32);

pub trait DrawBackend {
    // ── Frame lifecycle ─────────────────────────────────────

    /// Begin a new frame. Clears the canvas and sets up the coordinate space.
    /// `width` and `height` are in logical (CSS) pixels.
    fn begin_frame(&mut self, width: f64, height: f64);

    /// End the current frame. Flushes any pending draw commands.
    fn end_frame(&mut self);

    /// Set scale factors for HiDPI rendering.
    /// `sx` and `sy` are horizontal and vertical pixel ratios respectively.
    /// On most displays they are equal, but some displays may differ.
    fn set_scale(&mut self, sx: f64, sy: f64);

    // ── Rectangles ──────────────────────────────────────────

    /// Fill a rectangle with a solid color.
    fn fill_rect(&mut self, x: f64, y: f64, w: f64, h: f64, color: Color);

    /// Fill a rectangle with a vertical linear gradient.
    fn fill_rect_gradient(
        &mut self,
        x: f64,
        y: f64,
        w: f64,
        h: f64,
        y_start: f64,
        y_end: f64,
        stops: &[GradientStop],
    );

    // ── Lines ───────────────────────────────────────────────

    /// Stroke a single line segment.
    fn stroke_line(&mut self, x0: f64, y0: f64, x1: f64, y1: f64, color: Color, width: f64);

    /// Stroke a dashed line segment.
    fn stroke_dashed_line(
        &mut self,
        x0: f64,
        y0: f64,
        x1: f64,
        y1: f64,
        color: Color,
        width: f64,
        dash_len: f64,
        gap_len: f64,
    );

    // ── Paths ───────────────────────────────────────────────

    /// Stroke a polyline (connected line segments, not closed).
    fn stroke_path(&mut self, points: &[(f64, f64)], color: Color, width: f64);

    /// Fill a closed polygon with a solid color.
    fn fill_path(&mut self, points: &[(f64, f64)], color: Color);

    /// Fill a closed polygon with a vertical linear gradient.
    fn fill_path_gradient(
        &mut self,
        points: &[(f64, f64)],
        y_start: f64,
        y_end: f64,
        stops: &[GradientStop],
    );

    // ── Circles ─────────────────────────────────────────────

    /// Fill a circle with a solid color.
    fn fill_circle(&mut self, cx: f64, cy: f64, radius: f64, color: Color);

    // ── Text ────────────────────────────────────────────────

    /// Draw text at (x, y) where y is the baseline.
    fn draw_text(&mut self, text: &str, x: f64, y: f64, font_size: f64, color: Color);

    /// Measure text width in logical pixels.
    fn measure_text(&self, text: &str, font_size: f64) -> f64;
}

// ── Pixel-snap helper ───────────────────────────────────────

/// Snap a coordinate to the nearest device-pixel center for crisp 1px lines.
///
/// When a 1px line is drawn at an integer coordinate, it straddles two physical
/// pixels and gets anti-aliased into a blurry 2px line. By offsetting to the
/// pixel center (0.5 device pixels), the line lands entirely within one pixel.
///
/// # Examples
/// ```
/// // At 2x scale: snap(100.3, 2.0) → 100.25 → maps to device pixel 200.5
/// // At 1x scale: snap(100.3, 1.0) → 100.5
/// ```
pub fn snap(coord: f64, scale: f64) -> f64 {
    if scale <= 0.0 {
        return coord;
    }
    (coord * scale).round() / scale + 0.5 / scale
}

/// Snap a coordinate for a horizontal line (uses vertical scale).
pub fn snap_y(coord: f64, sy: f64) -> f64 {
    snap(coord, sy)
}

/// Snap a coordinate for a vertical line (uses horizontal scale).
pub fn snap_x(coord: f64, sx: f64) -> f64 {
    snap(coord, sx)
}
