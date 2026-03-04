/// Canvas2DBackend — implements DrawBackend for browser Canvas 2D API.
///
/// WASM-only: uses web_sys::CanvasRenderingContext2d for rendering.
/// Follows fancy-canvas patterns: bitmap vs media coordinate spaces,
/// ctx.save()/restore() for coordinate switches, proper HiDPI handling.
use wasm_bindgen::JsCast;
use wasm_bindgen::JsValue;
use web_sys::CanvasRenderingContext2d;

use crate::draw_backend::{Color, DrawBackend, GradientStop};

/// Canvas 2D backend for WASM — renders to a browser <canvas> element.
pub struct Canvas2DBackend {
    ctx: CanvasRenderingContext2d,
    width: f64,
    height: f64,
    scale_x: f64,
    scale_y: f64,
    // Offscreen canvas for cached bottom scene (fancy-canvas pattern)
    cached_canvas: Option<web_sys::OffscreenCanvas>,
    cached_ctx: Option<web_sys::OffscreenCanvasRenderingContext2d>,
}

impl Canvas2DBackend {
    /// Create a new Canvas 2D backend from a rendering context.
    pub fn new(ctx: CanvasRenderingContext2d) -> Self {
        Canvas2DBackend {
            ctx,
            width: 0.0,
            height: 0.0,
            scale_x: 1.0,
            scale_y: 1.0,
            cached_canvas: None,
            cached_ctx: None,
        }
    }

    /// Get the underlying context for direct access if needed.
    pub fn context(&self) -> &CanvasRenderingContext2d {
        &self.ctx
    }

    /// Ensure the offscreen canvas exists and matches the current bitmap size.
    fn ensure_offscreen(&mut self) {
        let bw = (self.width * self.scale_x) as u32;
        let bh = (self.height * self.scale_y) as u32;

        let needs_create = match &self.cached_canvas {
            Some(c) => c.width() != bw || c.height() != bh,
            None => true,
        };

        if needs_create {
            let canvas = web_sys::OffscreenCanvas::new(bw.max(1), bh.max(1))
                .expect("Failed to create OffscreenCanvas");
            let ctx = canvas
                .get_context("2d")
                .unwrap()
                .unwrap()
                .dyn_into::<web_sys::OffscreenCanvasRenderingContext2d>()
                .unwrap();
            self.cached_canvas = Some(canvas);
            self.cached_ctx = Some(ctx);
        }
    }

    /// Snapshot the current main canvas content to the offscreen cache.
    /// Call this after rendering the bottom scene to freeze it for reuse.
    pub fn snapshot_to_cache(&mut self) {
        self.ensure_offscreen();
        if let Some(ref ctx) = self.cached_ctx {
            // Get the main canvas element from the 2d context
            let main_canvas = self.ctx.canvas().unwrap();
            ctx.set_transform(1.0, 0.0, 0.0, 1.0, 0.0, 0.0).ok();
            ctx.draw_image_with_html_canvas_element(&main_canvas, 0.0, 0.0)
                .ok();
        }
    }

    /// Begin an overlay frame — sets up scaling WITHOUT clearing the canvas.
    /// Use this to composite crosshair on top of an existing bottom scene.
    pub fn begin_overlay_frame(&mut self, width: f64, height: f64) {
        self.width = width;
        self.height = height;
        // Don't clear — just set up the scale transform for drawing
        self.ctx.save();
        self.ctx.scale(self.scale_x, self.scale_y).ok();
    }

    /// Blit the cached offscreen canvas onto the main canvas.
    pub fn blit_cached(&mut self) {
        if let Some(ref canvas) = self.cached_canvas {
            self.ctx.set_transform(1.0, 0.0, 0.0, 1.0, 0.0, 0.0).ok();
            self.ctx
                .draw_image_with_offscreen_canvas(canvas, 0.0, 0.0)
                .ok();
        }
    }
}

// ── Helper functions ────────────────────────────────────────

/// Convert Color to a CSS rgba string.
fn color_to_css(c: Color) -> String {
    format!(
        "rgba({},{},{},{})",
        (c[0] * 255.0).round() as u8,
        (c[1] * 255.0).round() as u8,
        (c[2] * 255.0).round() as u8,
        c[3]
    )
}

impl DrawBackend for Canvas2DBackend {
    fn begin_frame(&mut self, width: f64, height: f64) {
        self.width = width;
        self.height = height;

        // Reset transform and clear in bitmap space
        self.ctx.set_transform(1.0, 0.0, 0.0, 1.0, 0.0, 0.0).ok();
        let bw = width * self.scale_x;
        let bh = height * self.scale_y;
        self.ctx.clear_rect(0.0, 0.0, bw, bh);

        // Apply scale for media → bitmap coordinate mapping
        self.ctx.save();
        self.ctx.scale(self.scale_x, self.scale_y).ok();
    }

    fn end_frame(&mut self) {
        self.ctx.restore();
    }

    fn set_scale(&mut self, sx: f64, sy: f64) {
        self.scale_x = sx;
        self.scale_y = sy;
    }

    fn fill_rect(&mut self, x: f64, y: f64, w: f64, h: f64, color: Color) {
        self.ctx.set_fill_style_str(&color_to_css(color));
        self.ctx.fill_rect(x, y, w, h);
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
        let gradient = self.ctx.create_linear_gradient(x, y_start, x, y_end);
        for (color, offset) in stops {
            gradient.add_color_stop(*offset, &color_to_css(*color)).ok();
        }
        self.ctx.set_fill_style_canvas_gradient(&gradient);
        self.ctx.fill_rect(x, y, w, h);
    }

    fn stroke_line(&mut self, x0: f64, y0: f64, x1: f64, y1: f64, color: Color, width: f64) {
        self.ctx.set_stroke_style_str(&color_to_css(color));
        self.ctx.set_line_width(width);
        self.ctx
            .set_line_dash(&JsValue::from(js_sys::Array::new()))
            .ok();
        self.ctx.begin_path();
        self.ctx.move_to(x0, y0);
        self.ctx.line_to(x1, y1);
        self.ctx.stroke();
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
        self.ctx.set_stroke_style_str(&color_to_css(color));
        self.ctx.set_line_width(width);
        let dash_array = js_sys::Array::new();
        dash_array.push(&JsValue::from_f64(dash_len));
        dash_array.push(&JsValue::from_f64(gap_len));
        self.ctx.set_line_dash(&dash_array).ok();
        self.ctx.begin_path();
        self.ctx.move_to(x0, y0);
        self.ctx.line_to(x1, y1);
        self.ctx.stroke();
        // Reset dash
        self.ctx
            .set_line_dash(&JsValue::from(js_sys::Array::new()))
            .ok();
    }

    fn stroke_path(&mut self, points: &[(f64, f64)], color: Color, width: f64) {
        if points.len() < 2 {
            return;
        }
        self.ctx.set_stroke_style_str(&color_to_css(color));
        self.ctx.set_line_width(width);
        self.ctx
            .set_line_dash(&JsValue::from(js_sys::Array::new()))
            .ok();
        self.ctx.begin_path();
        self.ctx.move_to(points[0].0, points[0].1);
        for &(x, y) in &points[1..] {
            self.ctx.line_to(x, y);
        }
        self.ctx.stroke();
    }

    fn fill_path(&mut self, points: &[(f64, f64)], color: Color) {
        if points.len() < 3 {
            return;
        }
        self.ctx.set_fill_style_str(&color_to_css(color));
        self.ctx.begin_path();
        self.ctx.move_to(points[0].0, points[0].1);
        for &(x, y) in &points[1..] {
            self.ctx.line_to(x, y);
        }
        self.ctx.close_path();
        self.ctx.fill();
    }

    fn fill_path_gradient(
        &mut self,
        points: &[(f64, f64)],
        y_start: f64,
        y_end: f64,
        stops: &[GradientStop],
    ) {
        if points.len() < 3 {
            return;
        }
        // Get bounding x range for the gradient line x position
        let x_mid = points.iter().map(|(x, _)| *x).sum::<f64>() / points.len() as f64;
        let gradient = self
            .ctx
            .create_linear_gradient(x_mid, y_start, x_mid, y_end);
        for (color, offset) in stops {
            gradient.add_color_stop(*offset, &color_to_css(*color)).ok();
        }
        self.ctx.set_fill_style_canvas_gradient(&gradient);
        self.ctx.begin_path();
        self.ctx.move_to(points[0].0, points[0].1);
        for &(x, y) in &points[1..] {
            self.ctx.line_to(x, y);
        }
        self.ctx.close_path();
        self.ctx.fill();
    }

    fn fill_circle(&mut self, cx: f64, cy: f64, radius: f64, color: Color) {
        self.ctx.set_fill_style_str(&color_to_css(color));
        self.ctx.begin_path();
        self.ctx
            .arc(cx, cy, radius, 0.0, std::f64::consts::PI * 2.0)
            .ok();
        self.ctx.fill();
    }

    fn draw_text(&mut self, text: &str, x: f64, y: f64, font_size: f64, color: Color) {
        self.ctx.set_font(&format!("{}px sans-serif", font_size));
        self.ctx.set_fill_style_str(&color_to_css(color));
        self.ctx.set_text_baseline("alphabetic");
        self.ctx.fill_text(text, x, y).ok();
    }

    fn measure_text(&self, text: &str, font_size: f64) -> f64 {
        self.ctx.set_font(&format!("{}px sans-serif", font_size));
        self.ctx
            .measure_text(text)
            .map(|m| m.width())
            .unwrap_or(text.len() as f64 * font_size * 0.6)
    }

    fn clip_rect(&mut self, x: f64, y: f64, w: f64, h: f64) {
        self.ctx.save();
        self.ctx.begin_path();
        self.ctx.rect(x, y, w, h);
        self.ctx.clip();
    }

    fn restore_clip(&mut self) {
        self.ctx.restore();
    }
}
