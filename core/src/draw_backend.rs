/// DrawBackend trait — abstract rendering primitives for chart drawing.
///
/// All drawing functions in chart_renderer.rs are generic over `impl DrawBackend`,
/// enabling multiple rendering backends: Vello (WebGPU/Metal), Canvas 2D, WebGL (femtovg).
///
/// Colors are `[f32; 4]` RGBA (0.0–1.0). Coordinates are f64 in logical (pre-scale) space.

/// A color in RGBA format, each component 0.0–1.0.
pub type Color4 = [f32; 4];

/// A gradient stop: (color, offset). Offset is 0.0 (start) to 1.0 (end).
pub type GradientStop = (Color4, f32);

pub trait DrawBackend {
    // ── Rectangles ──────────────────────────────────────────

    /// Fill a rectangle with a solid color.
    fn fill_rect(&mut self, x: f64, y: f64, w: f64, h: f64, color: Color4);

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
    fn stroke_line(&mut self, x0: f64, y0: f64, x1: f64, y1: f64, color: Color4, width: f64);

    /// Stroke a dashed line segment.
    fn stroke_dashed_line(
        &mut self,
        x0: f64,
        y0: f64,
        x1: f64,
        y1: f64,
        color: Color4,
        width: f64,
        dash_len: f64,
        gap_len: f64,
    );

    // ── Paths ───────────────────────────────────────────────

    /// Stroke a polyline (connected line segments, not closed).
    fn stroke_path(&mut self, points: &[(f64, f64)], color: Color4, width: f64);

    /// Fill a closed polygon with a solid color.
    fn fill_path(&mut self, points: &[(f64, f64)], color: Color4);

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
    fn fill_circle(&mut self, cx: f64, cy: f64, radius: f64, color: Color4);

    // ── Text ────────────────────────────────────────────────

    /// Draw text at (x, y) where y is the baseline.
    fn draw_text(&mut self, text: &str, x: f64, y: f64, font_size: f64, color: Color4);

    /// Measure text width in logical pixels.
    fn measure_text(&self, text: &str, font_size: f64) -> f64;

    // ── Frame lifecycle ─────────────────────────────────────

    /// Set the global scale factor (for HiDPI).
    fn set_scale(&mut self, scale: f64);
}
