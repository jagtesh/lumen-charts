/// Canvas2DRenderer — immediate-mode Renderer for browser Canvas 2D API.
///
/// WASM-only. Every render call draws directly to the canvas.
/// No scene caching — Canvas 2D is fast enough for chart rendering
/// without compositional optimizations.
use crate::backends::canvas2d::Canvas2DBackend;
use crate::chart_renderer::{render_bottom_scene, render_crosshair_scene};
use crate::chart_state::ChartState;
use crate::draw_backend::DrawBackend;
use crate::invalidation::InvalidationLevel;
use crate::renderers::Renderer;
use web_sys::CanvasRenderingContext2d;

/// Canvas 2D rendering pipeline for WASM.
pub struct Canvas2DRenderer {
    pub backend: Canvas2DBackend,
}

impl Canvas2DRenderer {
    /// Create a new Canvas 2D renderer from a rendering context.
    pub fn new(ctx: CanvasRenderingContext2d) -> Self {
        Canvas2DRenderer {
            backend: Canvas2DBackend::new(ctx),
        }
    }
}

impl Renderer for Canvas2DRenderer {
    fn render(&mut self, state: &mut ChartState, level: InvalidationLevel) {
        let w = state.layout.width as f64;
        let h = state.layout.height as f64;

        self.backend.begin_frame(w, h);

        if level.needs_bottom_scene() || level >= InvalidationLevel::Light {
            render_bottom_scene(&mut self.backend, state);
            state.bottom_render_count += 1;
        }

        render_crosshair_scene(&mut self.backend, state);
        state.crosshair_render_count += 1;

        self.backend.end_frame();
    }

    fn resize(&mut self, width: u32, height: u32, scale_factor: f64) {
        // Update the backend's scale so HiDPI mapping works
        self.backend.set_scale(scale_factor, scale_factor);

        // The WASM SDK will also update the canvas element's bitmap dimensions
        let _ = (width, height);
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
}
