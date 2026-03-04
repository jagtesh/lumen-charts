/// Drawing backends — implementations of the `DrawBackend` trait.
///
/// Each backend translates abstract drawing primitives (fill_rect, stroke_line, etc.)
/// into a specific rendering API. The chart renderer is generic over `impl DrawBackend`,
/// giving compile-time dispatch with zero overhead.
pub mod vello;

#[cfg(target_arch = "wasm32")]
pub mod canvas2d;

// Re-export for convenience
#[cfg(target_arch = "wasm32")]
pub use self::canvas2d::Canvas2DBackend;
pub use self::vello::VelloBackend;
