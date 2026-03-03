/// VelloBackend — implements DrawBackend for Vello's Scene graph.
///
/// This wraps the existing rendering approach: build a vello::Scene,
/// which is later submitted to GPU via VelloRenderer::render_to_surface.
use vello::kurbo::{Affine, BezPath, Circle, Line, Rect as KurboRect, Stroke};
use vello::peniko::{self, Brush, Color, Fill, Gradient};
use vello::Scene;

use crate::draw_backend::{Color as ChartColor, DrawBackend, GradientStop};
use crate::text_render;

/// Vello-based backend: accumulates draw commands into a Scene.
pub struct VelloBackend {
    pub scene: Scene,
    scale: Affine,
    scale_x: f64,
    scale_y: f64,
    font: peniko::Font,
}

impl VelloBackend {
    pub fn new() -> Self {
        VelloBackend {
            scene: Scene::new(),
            scale: Affine::IDENTITY,
            scale_x: 1.0,
            scale_y: 1.0,
            font: text_render::chart_font(),
        }
    }

    /// Reset the scene for a new frame.
    pub fn reset(&mut self) {
        self.scene.reset();
    }

    /// Take the built scene (for render_to_surface).
    pub fn scene(&self) -> &Scene {
        &self.scene
    }

    /// Get a mutable ref to the scene (for appending cached scenes).
    pub fn scene_mut(&mut self) -> &mut Scene {
        &mut self.scene
    }
}

fn c4_to_color(c: ChartColor) -> Color {
    Color::new(c.0)
}

fn stops_to_gradient(y_start: f64, y_end: f64, stops: &[GradientStop]) -> Brush {
    let grad_stops: Vec<(f32, Color)> = stops
        .iter()
        .map(|(c, offset)| (*offset, c4_to_color(*c)))
        .collect();
    Brush::Gradient(
        Gradient::new_linear((0.0, y_start), (0.0, y_end)).with_stops(grad_stops.as_slice()),
    )
}

fn build_path(points: &[(f64, f64)], close: bool) -> BezPath {
    let mut path = BezPath::new();
    if let Some(&(x, y)) = points.first() {
        path.move_to((x, y));
        for &(px, py) in &points[1..] {
            path.line_to((px, py));
        }
        if close {
            path.close_path();
        }
    }
    path
}

impl DrawBackend for VelloBackend {
    fn begin_frame(&mut self, _width: f64, _height: f64) {
        self.scene.reset();
    }

    fn end_frame(&mut self) {
        // No-op for Vello — scene is consumed by render_to_surface.
    }

    fn set_scale(&mut self, sx: f64, sy: f64) {
        self.scale_x = sx;
        self.scale_y = sy;
        // Vello uses a single Affine transform.
        // For non-uniform scaling, we compose a scale affine.
        self.scale = Affine::scale_non_uniform(sx, sy);
    }

    fn fill_rect(&mut self, x: f64, y: f64, w: f64, h: f64, color: ChartColor) {
        self.scene.fill(
            Fill::NonZero,
            self.scale,
            c4_to_color(color),
            None,
            &KurboRect::new(x, y, x + w, y + h),
        );
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
        let brush = stops_to_gradient(y_start, y_end, stops);
        self.scene.fill(
            Fill::NonZero,
            self.scale,
            &brush,
            None,
            &KurboRect::new(x, y, x + w, y + h),
        );
    }

    fn stroke_line(&mut self, x0: f64, y0: f64, x1: f64, y1: f64, color: ChartColor, width: f64) {
        self.scene.stroke(
            &Stroke::new(width),
            self.scale,
            c4_to_color(color),
            None,
            &Line::new((x0, y0), (x1, y1)),
        );
    }

    fn stroke_dashed_line(
        &mut self,
        x0: f64,
        y0: f64,
        x1: f64,
        y1: f64,
        color: ChartColor,
        width: f64,
        dash_len: f64,
        gap_len: f64,
    ) {
        let stroke = Stroke::new(width).with_dashes(0.0, [dash_len, gap_len]);
        self.scene.stroke(
            &stroke,
            self.scale,
            c4_to_color(color),
            None,
            &Line::new((x0, y0), (x1, y1)),
        );
    }

    fn stroke_path(&mut self, points: &[(f64, f64)], color: ChartColor, width: f64) {
        let path = build_path(points, false);
        self.scene.stroke(
            &Stroke::new(width),
            self.scale,
            c4_to_color(color),
            None,
            &path,
        );
    }

    fn fill_path(&mut self, points: &[(f64, f64)], color: ChartColor) {
        let path = build_path(points, true);
        self.scene
            .fill(Fill::NonZero, self.scale, c4_to_color(color), None, &path);
    }

    fn fill_path_gradient(
        &mut self,
        points: &[(f64, f64)],
        y_start: f64,
        y_end: f64,
        stops: &[GradientStop],
    ) {
        let path = build_path(points, true);
        let brush = stops_to_gradient(y_start, y_end, stops);
        self.scene
            .fill(Fill::NonZero, self.scale, &brush, None, &path);
    }

    fn fill_circle(&mut self, cx: f64, cy: f64, radius: f64, color: ChartColor) {
        self.scene.fill(
            Fill::NonZero,
            self.scale,
            c4_to_color(color),
            None,
            &Circle::new((cx, cy), radius),
        );
    }

    fn draw_text(&mut self, text: &str, x: f64, y: f64, font_size: f64, color: ChartColor) {
        text_render::draw_text(
            &mut self.scene,
            &self.font,
            text,
            x,
            y,
            font_size as f32,
            c4_to_color(color),
            self.scale,
        );
    }

    fn measure_text(&self, text: &str, font_size: f64) -> f64 {
        text_render::measure_text(&self.font, text, font_size as f32) as f64
    }
}
