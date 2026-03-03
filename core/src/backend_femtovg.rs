/// FemtovgBackend — implements DrawBackend for femtovg (NanoVG-inspired 2D graphics).
///
/// Works on both native (desktop OpenGL via glow) and WASM (WebGL2 via glow).
/// This is an alternative to Vello for platforms where GPU compute shaders
/// aren't available (e.g., older machines, WebGL-only browsers).
///
/// Gated behind the `femtovg-backend` feature flag.
use femtovg::{renderer::OpenGl, Canvas, Color as FvgColor, Paint, Path as FvgPath};

use crate::draw_backend::{Color, DrawBackend, GradientStop};

/// femtovg-based backend: renders via OpenGL / WebGL2.
pub struct FemtovgBackend {
    canvas: Canvas<OpenGl>,
    width: f64,
    height: f64,
    scale_x: f32,
    scale_y: f32,
}

impl FemtovgBackend {
    /// Create a new femtovg backend from an existing Canvas<OpenGl>.
    ///
    /// The caller is responsible for creating the OpenGl renderer
    /// (via glow context) and the initial Canvas.
    pub fn new(canvas: Canvas<OpenGl>) -> Self {
        FemtovgBackend {
            canvas,
            width: 0.0,
            height: 0.0,
            scale_x: 1.0,
            scale_y: 1.0,
        }
    }

    /// Get a reference to the underlying canvas.
    pub fn canvas(&self) -> &Canvas<OpenGl> {
        &self.canvas
    }

    /// Get a mutable reference to the underlying canvas.
    pub fn canvas_mut(&mut self) -> &mut Canvas<OpenGl> {
        &mut self.canvas
    }
}

// ── Helper functions ────────────────────────────────────────

fn c4_to_fvg(c: Color) -> FvgColor {
    FvgColor::rgbaf(c[0], c[1], c[2], c[3])
}

fn build_fvg_path(points: &[(f64, f64)], close: bool) -> FvgPath {
    let mut path = FvgPath::new();
    if let Some(&(x, y)) = points.first() {
        path.move_to(x as f32, y as f32);
        for &(px, py) in &points[1..] {
            path.line_to(px as f32, py as f32);
        }
        if close {
            path.close();
        }
    }
    path
}

impl DrawBackend for FemtovgBackend {
    fn begin_frame(&mut self, width: f64, height: f64) {
        self.width = width;
        self.height = height;
        let bw = (width as f32 * self.scale_x) as u32;
        let bh = (height as f32 * self.scale_y) as u32;
        self.canvas.set_size(bw, bh, 1.0);
        self.canvas
            .clear_rect(0, 0, bw, bh, FvgColor::rgbaf(0.0, 0.0, 0.0, 0.0));
        self.canvas.save();
        self.canvas.scale(self.scale_x, self.scale_y);
    }

    fn end_frame(&mut self) {
        self.canvas.restore();
        self.canvas.flush();
    }

    fn set_scale(&mut self, sx: f64, sy: f64) {
        self.scale_x = sx as f32;
        self.scale_y = sy as f32;
    }

    fn fill_rect(&mut self, x: f64, y: f64, w: f64, h: f64, color: Color) {
        let mut path = FvgPath::new();
        path.rect(x as f32, y as f32, w as f32, h as f32);
        let paint = Paint::color(c4_to_fvg(color));
        self.canvas.fill_path(&path, &paint);
    }

    fn fill_rect_gradient(
        &mut self,
        x: f64,
        y: f64,
        w: f64,
        h: f64,
        y_start: f64,
        y_end: f64,
        stops: &[GradientStop],
    ) {
        if stops.len() < 2 {
            return;
        }
        let mut path = FvgPath::new();
        path.rect(x as f32, y as f32, w as f32, h as f32);

        // femtovg supports two-stop linear gradients natively
        let start_color = c4_to_fvg(stops[0].0);
        let end_color = c4_to_fvg(stops[stops.len() - 1].0);
        let paint = Paint::linear_gradient(
            x as f32,
            y_start as f32,
            x as f32,
            y_end as f32,
            start_color,
            end_color,
        );
        self.canvas.fill_path(&path, &paint);
    }

    fn stroke_line(&mut self, x0: f64, y0: f64, x1: f64, y1: f64, color: Color, width: f64) {
        let mut path = FvgPath::new();
        path.move_to(x0 as f32, y0 as f32);
        path.line_to(x1 as f32, y1 as f32);
        let mut paint = Paint::color(c4_to_fvg(color));
        paint.set_line_width(width as f32);
        self.canvas.stroke_path(&path, &paint);
    }

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
    ) {
        // femtovg doesn't have native dash support — simulate with segments
        let dx = x1 - x0;
        let dy = y1 - y0;
        let len = (dx * dx + dy * dy).sqrt();
        if len <= 0.0 {
            return;
        }
        let ux = dx / len;
        let uy = dy / len;
        let segment = dash_len + gap_len;

        let mut paint = Paint::color(c4_to_fvg(color));
        paint.set_line_width(width as f32);

        let mut t = 0.0;
        while t < len {
            let end = (t + dash_len).min(len);
            let mut path = FvgPath::new();
            path.move_to((x0 + ux * t) as f32, (y0 + uy * t) as f32);
            path.line_to((x0 + ux * end) as f32, (y0 + uy * end) as f32);
            self.canvas.stroke_path(&path, &paint);
            t += segment;
        }
    }

    fn stroke_path(&mut self, points: &[(f64, f64)], color: Color, width: f64) {
        if points.len() < 2 {
            return;
        }
        let path = build_fvg_path(points, false);
        let mut paint = Paint::color(c4_to_fvg(color));
        paint.set_line_width(width as f32);
        self.canvas.stroke_path(&path, &paint);
    }

    fn fill_path(&mut self, points: &[(f64, f64)], color: Color) {
        if points.len() < 3 {
            return;
        }
        let path = build_fvg_path(points, true);
        let paint = Paint::color(c4_to_fvg(color));
        self.canvas.fill_path(&path, &paint);
    }

    fn fill_path_gradient(
        &mut self,
        points: &[(f64, f64)],
        y_start: f64,
        y_end: f64,
        stops: &[GradientStop],
    ) {
        if points.len() < 3 || stops.len() < 2 {
            return;
        }
        let path = build_fvg_path(points, true);
        let x_mid = points.iter().map(|(x, _)| *x).sum::<f64>() / points.len() as f64;
        let start_color = c4_to_fvg(stops[0].0);
        let end_color = c4_to_fvg(stops[stops.len() - 1].0);
        let paint = Paint::linear_gradient(
            x_mid as f32,
            y_start as f32,
            x_mid as f32,
            y_end as f32,
            start_color,
            end_color,
        );
        self.canvas.fill_path(&path, &paint);
    }

    fn fill_circle(&mut self, cx: f64, cy: f64, radius: f64, color: Color) {
        let mut path = FvgPath::new();
        path.circle(cx as f32, cy as f32, radius as f32);
        let paint = Paint::color(c4_to_fvg(color));
        self.canvas.fill_path(&path, &paint);
    }

    fn draw_text(&mut self, text: &str, x: f64, y: f64, font_size: f64, color: Color) {
        let mut paint = Paint::color(c4_to_fvg(color));
        paint.set_font_size(font_size as f32);
        // Note: font must be loaded into the canvas separately via canvas.add_font()
        let _ = self.canvas.fill_text(x as f32, y as f32, text, &paint);
    }

    fn measure_text(&self, text: &str, font_size: f64) -> f64 {
        let mut paint = Paint::color(FvgColor::white());
        paint.set_font_size(font_size as f32);
        match self.canvas.measure_text(0.0, 0.0, text, &paint) {
            Ok(metrics) => metrics.width() as f64,
            Err(_) => text.len() as f64 * font_size * 0.6,
        }
    }
}
