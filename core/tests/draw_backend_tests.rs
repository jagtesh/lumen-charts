//! Tests for the DrawBackend trait, pixel-snap helpers, and backend lifecycle.
//!
//! These tests verify:
//! - snap(), snap_x(), snap_y() pixel-snapping correctness
//! - begin_frame / end_frame lifecycle on VelloBackend
//! - set_scale(sx, sy) non-uniform scaling
//! - DrawBackend trait method coverage on VelloBackend

use lumen_charts::backend_vello::VelloBackend;
use lumen_charts::draw_backend::{snap, snap_x, snap_y, Color4, DrawBackend};

// =============================================================================
// Pixel-snap helper tests
// =============================================================================

#[test]
fn snap_at_1x_rounds_to_half_pixel() {
    // At 1x scale, snap to 0.5 pixel centers
    // snap(100.0, 1.0) = round(100.0) / 1.0 + 0.5 / 1.0 = 100.5
    assert_eq!(snap(100.0, 1.0), 100.5);
}

#[test]
fn snap_at_1x_fractional_coord() {
    // 100.3 at 1x: round(100.3) = 100.0, 100.0 + 0.5 = 100.5
    assert_eq!(snap(100.3, 1.0), 100.5);
    // 100.7 at 1x: round(100.7) = 101.0, 101.0 + 0.5 = 101.5
    assert_eq!(snap(100.7, 1.0), 101.5);
}

#[test]
fn snap_at_2x_halves_pixel_offset() {
    // At 2x: snap(100.0, 2.0) = round(200.0) / 2.0 + 0.5 / 2.0 = 100.0 + 0.25 = 100.25
    assert_eq!(snap(100.0, 2.0), 100.25);
}

#[test]
fn snap_at_2x_fractional_coord() {
    // 100.3 at 2x: round(200.6) = 201, 201/2 + 0.25 = 100.5 + 0.25 = 100.75
    assert_eq!(snap(100.3, 2.0), 100.75);
}

#[test]
fn snap_at_3x_scale() {
    // At 3x: snap(50.0, 3.0)
    // round(50.0 * 3.0) = round(150.0) = 150
    // 150 / 3.0 + 0.5 / 3.0 = 50.0 + 0.1666...
    let result = snap(50.0, 3.0);
    let expected = 50.0 + 0.5 / 3.0;
    assert!(
        (result - expected).abs() < 1e-10,
        "got {}, expected {}",
        result,
        expected
    );
}

#[test]
fn snap_zero_coord() {
    assert_eq!(snap(0.0, 1.0), 0.5);
    assert_eq!(snap(0.0, 2.0), 0.25);
}

#[test]
fn snap_negative_coord() {
    // snap(-1.0, 1.0) = round(-1.0) / 1.0 + 0.5 = -1.0 + 0.5 = -0.5
    assert_eq!(snap(-1.0, 1.0), -0.5);
}

#[test]
fn snap_zero_scale_returns_original() {
    // Edge case: scale <= 0 should return coord unchanged
    assert_eq!(snap(100.0, 0.0), 100.0);
    assert_eq!(snap(100.0, -1.0), 100.0);
}

#[test]
fn snap_x_and_snap_y_are_aliases() {
    // snap_x and snap_y should produce identical results for same inputs
    let coord = 123.456;
    let scale = 2.0;
    assert_eq!(snap_x(coord, scale), snap(coord, scale));
    assert_eq!(snap_y(coord, scale), snap(coord, scale));
}

// =============================================================================
// VelloBackend lifecycle tests
// =============================================================================

#[test]
fn vello_begin_frame_resets_scene() {
    let mut backend = VelloBackend::new();

    // Draw something
    backend.set_scale(1.0, 1.0);
    backend.fill_rect(0.0, 0.0, 100.0, 100.0, [1.0, 0.0, 0.0, 1.0]);
    assert!(
        !backend.scene.encoding().is_empty(),
        "scene should have content after draw"
    );

    // begin_frame should reset
    backend.begin_frame(800.0, 600.0);
    assert!(
        backend.scene.encoding().is_empty(),
        "scene should be empty after begin_frame"
    );
}

#[test]
fn vello_end_frame_is_noop() {
    let mut backend = VelloBackend::new();
    backend.begin_frame(800.0, 600.0);
    backend.fill_rect(0.0, 0.0, 50.0, 50.0, [0.0, 1.0, 0.0, 1.0]);
    let before = backend.scene.encoding().is_empty();

    backend.end_frame();
    let after = backend.scene.encoding().is_empty();

    // end_frame should not clear the scene (it's consumed by render_to_surface later)
    assert_eq!(before, after, "end_frame should not change scene state");
}

#[test]
fn vello_set_scale_uniform() {
    let mut backend = VelloBackend::new();
    backend.set_scale(2.0, 2.0);

    // Drawing after scale should not panic
    backend.fill_rect(0.0, 0.0, 100.0, 100.0, [1.0, 1.0, 1.0, 1.0]);
    assert!(!backend.scene.encoding().is_empty());
}

#[test]
fn vello_set_scale_non_uniform() {
    let mut backend = VelloBackend::new();
    backend.set_scale(2.0, 3.0);

    // Drawing with non-uniform scale should not panic
    backend.fill_rect(0.0, 0.0, 100.0, 100.0, [1.0, 1.0, 1.0, 1.0]);
    backend.stroke_line(0.0, 0.0, 100.0, 100.0, [1.0, 0.0, 0.0, 1.0], 1.0);
    assert!(!backend.scene.encoding().is_empty());
}

// =============================================================================
// DrawBackend method coverage on VelloBackend
// =============================================================================

const RED: Color4 = [1.0, 0.0, 0.0, 1.0];
const GREEN: Color4 = [0.0, 1.0, 0.0, 1.0];
const BLUE: Color4 = [0.0, 0.0, 1.0, 1.0];
const WHITE: Color4 = [1.0, 1.0, 1.0, 1.0];

#[test]
fn vello_fill_rect_produces_content() {
    let mut b = VelloBackend::new();
    b.set_scale(1.0, 1.0);
    b.fill_rect(10.0, 20.0, 100.0, 50.0, RED);
    assert!(!b.scene.encoding().is_empty());
}

#[test]
fn vello_fill_rect_gradient_produces_content() {
    let mut b = VelloBackend::new();
    b.set_scale(1.0, 1.0);
    b.fill_rect_gradient(
        0.0,
        0.0,
        100.0,
        100.0,
        0.0,
        100.0,
        &[([1.0, 0.0, 0.0, 1.0], 0.0), ([0.0, 0.0, 1.0, 1.0], 1.0)],
    );
    assert!(!b.scene.encoding().is_empty());
}

#[test]
fn vello_stroke_line_produces_content() {
    let mut b = VelloBackend::new();
    b.set_scale(1.0, 1.0);
    b.stroke_line(0.0, 0.0, 100.0, 100.0, GREEN, 2.0);
    assert!(!b.scene.encoding().is_empty());
}

#[test]
fn vello_stroke_dashed_line_produces_content() {
    let mut b = VelloBackend::new();
    b.set_scale(1.0, 1.0);
    b.stroke_dashed_line(0.0, 50.0, 200.0, 50.0, BLUE, 1.0, 4.0, 4.0);
    assert!(!b.scene.encoding().is_empty());
}

#[test]
fn vello_stroke_path_produces_content() {
    let mut b = VelloBackend::new();
    b.set_scale(1.0, 1.0);
    let points = vec![(10.0, 10.0), (50.0, 80.0), (90.0, 10.0)];
    b.stroke_path(&points, RED, 1.5);
    assert!(!b.scene.encoding().is_empty());
}

#[test]
fn vello_fill_path_produces_content() {
    let mut b = VelloBackend::new();
    b.set_scale(1.0, 1.0);
    let points = vec![(0.0, 0.0), (100.0, 0.0), (50.0, 100.0)];
    b.fill_path(&points, GREEN);
    assert!(!b.scene.encoding().is_empty());
}

#[test]
fn vello_fill_path_gradient_produces_content() {
    let mut b = VelloBackend::new();
    b.set_scale(1.0, 1.0);
    let points = vec![(0.0, 0.0), (100.0, 0.0), (100.0, 100.0), (0.0, 100.0)];
    b.fill_path_gradient(
        &points,
        0.0,
        100.0,
        &[([1.0, 0.0, 0.0, 0.5], 0.0), ([0.0, 0.0, 1.0, 0.5], 1.0)],
    );
    assert!(!b.scene.encoding().is_empty());
}

#[test]
fn vello_fill_circle_produces_content() {
    let mut b = VelloBackend::new();
    b.set_scale(1.0, 1.0);
    b.fill_circle(50.0, 50.0, 25.0, BLUE);
    assert!(!b.scene.encoding().is_empty());
}

#[test]
fn vello_draw_text_does_not_panic() {
    let mut b = VelloBackend::new();
    b.set_scale(1.0, 1.0);
    // Verify draw_text runs without panicking.
    // Note: Vello text glyph encoding may be empty in test context
    // (no GPU, simplified font loading), so we just verify no panic.
    b.draw_text("Hello", 10.0, 20.0, 12.0, WHITE);
    b.draw_text("", 0.0, 0.0, 12.0, WHITE);
    b.draw_text("Long text with many characters", 0.0, 50.0, 14.0, RED);
}

#[test]
fn vello_measure_text_returns_positive() {
    let b = VelloBackend::new();
    let width = b.measure_text("Hello", 12.0);
    assert!(
        width > 0.0,
        "measure_text should return positive width, got {}",
        width
    );
}

#[test]
fn vello_measure_text_longer_string_is_wider() {
    let b = VelloBackend::new();
    let w1 = b.measure_text("Hi", 12.0);
    let w2 = b.measure_text("Hello World", 12.0);
    assert!(w2 > w1, "longer text should be wider: {} vs {}", w1, w2);
}

#[test]
fn vello_measure_text_empty_string() {
    let b = VelloBackend::new();
    let width = b.measure_text("", 12.0);
    assert!(width >= 0.0, "empty string should have non-negative width");
}

#[test]
fn vello_full_frame_lifecycle() {
    let mut b = VelloBackend::new();
    b.set_scale(2.0, 2.0);
    b.begin_frame(800.0, 600.0);

    // Draw various elements
    b.fill_rect(0.0, 0.0, 800.0, 600.0, [0.07, 0.07, 0.1, 1.0]);
    b.stroke_line(100.0, 100.0, 700.0, 100.0, RED, 1.0);
    b.fill_circle(400.0, 300.0, 50.0, GREEN);
    b.draw_text("Test", 10.0, 20.0, 14.0, WHITE);

    b.end_frame();

    // Scene should have content after a full lifecycle
    assert!(
        !b.scene.encoding().is_empty(),
        "scene should have content after full lifecycle"
    );
}
