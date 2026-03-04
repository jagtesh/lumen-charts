/// Canvas2DRenderer — immediate-mode Renderer for browser Canvas 2D API.
///
/// Uses offscreen canvas caching (fancy-canvas pattern):
/// - Bottom scene renders to main canvas, then gets copied to an offscreen cache
/// - On cursor-only frames, the cached bitmap is blitted back instantly
/// - Crosshair renders on top each frame
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

        if level.needs_bottom_scene() || level >= InvalidationLevel::Light {
            // Rebuild bottom scene: render to main canvas, then snapshot to offscreen
            self.backend.begin_frame(w, h);
            render_bottom_scene(&mut self.backend, state);
            self.backend.end_frame();

            // Snapshot the main canvas content to the offscreen cache
            self.backend.snapshot_to_cache();
            state.bottom_render_count += 1;

            // Now composite crosshair on top of the bottom scene
            // (begin a new frame that doesn't clear, but we need the scale)
            self.backend.begin_overlay_frame(w, h);
            render_crosshair_scene(&mut self.backend, state);
            self.backend.end_frame();
        } else {
            // Cursor-only: blit cached bottom scene, then draw crosshair on top
            self.backend.begin_frame(w, h);
            self.backend.blit_cached();
            self.backend.begin_overlay_frame(w, h);
            render_crosshair_scene(&mut self.backend, state);
            self.backend.end_frame();
        }

        state.crosshair_render_count += 1;
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
