/// Renderers — implementations of the `Renderer` trait.
///
/// Each renderer owns hardware-specific resources (GPU device, canvas context, etc.)
/// and handles the full render + present cycle. The `Chart` struct holds a
/// `Box<dyn Renderer>`, giving a single vtable call per frame.
use crate::chart_state::ChartState;
use crate::invalidation::InvalidationLevel;

/// Renderer trait — encapsulates a complete rendering pipeline.
///
/// Implementations own all hardware-specific resources (GPU device, surface,
/// GL context, canvas context, etc.) and handle drawing + presentation.
///
/// The `Chart` struct stores `Box<dyn Renderer>`, giving a single vtable call
/// per frame for render/resize. All actual drawing within each implementation
/// uses `impl DrawBackend` for compile-time dispatch.
pub trait Renderer {
    /// Render the chart at the given invalidation level.
    ///
    /// The implementation should:
    /// 1. Call `render_bottom_scene()` for Light/Full levels (or use cached)
    /// 2. Call `render_crosshair_scene()` always
    /// 3. Present the result to the screen/surface
    fn render(&mut self, state: &mut ChartState, level: InvalidationLevel);

    /// Handle viewport resize. Reconfigures internal surfaces/contexts.
    fn resize(&mut self, width: u32, height: u32, scale_factor: f64);

    /// Downcast to concrete type. Used when platform code needs direct
    /// access to hardware resources (e.g., Rust demo co-rendering egui
    /// with Vello on the same wgpu surface).
    fn as_any(&self) -> &dyn std::any::Any;

    /// Mutable downcast to concrete type.
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any;
}

pub mod vello;

#[cfg(target_arch = "wasm32")]
pub mod canvas2d;

// Re-export for convenience
#[cfg(target_arch = "wasm32")]
pub use self::canvas2d::Canvas2DRenderer;
pub use self::vello::VelloRenderer;
