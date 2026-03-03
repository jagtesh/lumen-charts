//! Integration tests for the invalidation system.
//!
//! These tests verify the full lifecycle of:
//! - Idle detection (no interaction → skip rendering)
//! - Scene caching (cursor-only → reuse bottom scene)
//! - Mask coalescing (multiple interactions merge correctly)
//! - Render counter accuracy

use lumen_charts::backend_vello::VelloBackend;
use lumen_charts::chart_model::{ChartData, OhlcBar};
use lumen_charts::chart_renderer::{render_bottom_scene, render_crosshair_scene};
use lumen_charts::chart_state::ChartState;
use lumen_charts::invalidation::InvalidationLevel;
use lumen_charts::sample_data::sample_data;

fn make_state() -> ChartState {
    let data = ChartData {
        bars: sample_data(),
    };
    ChartState::new(data, 800.0, 500.0, 1.0)
}

/// Simulate what chart_render does, but without GPU.
/// Returns the invalidation level that was consumed.
fn simulate_render(state: &mut ChartState) -> InvalidationLevel {
    let mask = state.consume_mask();
    let level = mask.global_level();

    if !mask.needs_redraw() {
        state.skipped_render_count += 1;
        return InvalidationLevel::None;
    }

    let mut backend = VelloBackend::new();

    if level.needs_bottom_scene() {
        render_bottom_scene(&mut backend, state);
        state.bottom_render_count += 1;
    }
    // For cursor-only, we skip bottom scene (would reuse cache in real impl)

    render_crosshair_scene(&mut backend, state);
    state.crosshair_render_count += 1;

    level
}

// =============================================================================
// Idle verification
// =============================================================================

#[test]
fn test_idle_after_initial_render_produces_no_work() {
    let mut state = make_state();
    // Initial render consumes Full mask
    let level = simulate_render(&mut state);
    assert_eq!(level, InvalidationLevel::Full);
    assert_eq!(state.bottom_render_count, 1);

    // Now idle — consecutive renders should skip
    for _ in 0..10 {
        let level = simulate_render(&mut state);
        assert_eq!(
            level,
            InvalidationLevel::None,
            "idle frame should produce None"
        );
    }
    assert_eq!(state.skipped_render_count, 10);
    assert_eq!(
        state.bottom_render_count, 1,
        "bottom scene should not have been rebuilt"
    );
    assert_eq!(
        state.crosshair_render_count, 1,
        "crosshair should not have been rendered"
    );
}

#[test]
fn test_idle_after_scroll_produces_no_work() {
    let mut state = make_state();
    simulate_render(&mut state); // consume initial Full

    // Scroll
    state.scroll(20.0, 0.0);
    let level = simulate_render(&mut state);
    assert_eq!(level, InvalidationLevel::Light);
    assert_eq!(state.bottom_render_count, 2);

    // Now idle
    let level = simulate_render(&mut state);
    assert_eq!(level, InvalidationLevel::None);
    assert_eq!(state.skipped_render_count, 1);
    assert_eq!(
        state.bottom_render_count, 2,
        "no extra bottom renders after idle"
    );
}

#[test]
fn test_idle_after_zoom_produces_no_work() {
    let mut state = make_state();
    simulate_render(&mut state);

    state.zoom(1.5, 400.0);
    simulate_render(&mut state);
    assert_eq!(state.bottom_render_count, 2);

    // Idle
    simulate_render(&mut state);
    assert_eq!(state.skipped_render_count, 1);
}

#[test]
fn test_hundred_idle_frames_zero_renders() {
    let mut state = make_state();
    simulate_render(&mut state); // initial

    for _ in 0..100 {
        simulate_render(&mut state);
    }
    assert_eq!(state.skipped_render_count, 100);
    assert_eq!(
        state.bottom_render_count, 1,
        "only the initial render built the bottom scene"
    );
    assert_eq!(
        state.crosshair_render_count, 1,
        "only the initial render drew crosshair"
    );
}

// =============================================================================
// Caching verification
// =============================================================================

#[test]
fn test_cursor_only_skips_bottom_scene() {
    let mut state = make_state();
    simulate_render(&mut state); // initial Full → bottom + crosshair
    assert_eq!(state.bottom_render_count, 1);

    // Mouse move in plot area → Cursor level
    let cx = state.layout.plot_area.x + 100.0;
    let cy = state.layout.plot_area.y + 100.0;
    state.pointer_move(cx, cy);

    let level = simulate_render(&mut state);
    assert_eq!(level, InvalidationLevel::Cursor);
    assert_eq!(
        state.bottom_render_count, 1,
        "cursor-only should NOT rebuild bottom"
    );
    assert_eq!(
        state.crosshair_render_count, 2,
        "crosshair SHOULD have been redrawn"
    );
}

#[test]
fn test_consecutive_cursor_moves_skip_bottom_scene() {
    let mut state = make_state();
    simulate_render(&mut state); // initial

    // 5 consecutive mouse moves
    for i in 0..5 {
        let cx = state.layout.plot_area.x + 50.0 + (i as f32 * 20.0);
        let cy = state.layout.plot_area.y + 100.0;
        state.pointer_move(cx, cy);
        let level = simulate_render(&mut state);
        assert_eq!(level, InvalidationLevel::Cursor);
    }

    assert_eq!(
        state.bottom_render_count, 1,
        "5 cursor moves should reuse same bottom"
    );
    assert_eq!(
        state.crosshair_render_count, 6,
        "initial + 5 cursor renders"
    );
}

#[test]
fn test_light_after_cursor_rebuilds_bottom() {
    let mut state = make_state();
    simulate_render(&mut state); // initial Full

    // Cursor move
    let cx = state.layout.plot_area.x + 100.0;
    let cy = state.layout.plot_area.y + 100.0;
    state.pointer_move(cx, cy);
    simulate_render(&mut state);
    assert_eq!(state.bottom_render_count, 1);

    // Scroll → Light
    state.scroll(10.0, 0.0);
    let level = simulate_render(&mut state);
    assert_eq!(level, InvalidationLevel::Light);
    assert_eq!(
        state.bottom_render_count, 2,
        "Light should rebuild bottom after cursor"
    );
}

#[test]
fn test_data_change_invalidates_cache() {
    let mut state = make_state();
    simulate_render(&mut state);

    // Data change → Light
    state.set_data(vec![
        OhlcBar {
            time: 1,
            open: 100.0,
            high: 110.0,
            low: 90.0,
            close: 105.0,
        },
        OhlcBar {
            time: 2,
            open: 105.0,
            high: 115.0,
            low: 95.0,
            close: 110.0,
        },
    ]);

    let level = simulate_render(&mut state);
    assert_eq!(level, InvalidationLevel::Light);
    assert_eq!(
        state.bottom_render_count, 2,
        "data change should rebuild bottom"
    );
}

// =============================================================================
// Coalescing verification
// =============================================================================

#[test]
fn test_multiple_interactions_coalesce_to_highest_level() {
    let mut state = make_state();
    simulate_render(&mut state);

    // Multiple interactions between renders
    let cx = state.layout.plot_area.x + 100.0;
    let cy = state.layout.plot_area.y + 100.0;
    state.pointer_move(cx, cy); // → Cursor
    state.scroll(5.0, 0.0); // → Light (upgrades)
    state.pointer_move(cx + 10.0, cy); // → stays Light (max)

    let level = simulate_render(&mut state);
    assert_eq!(level, InvalidationLevel::Light);
    assert_eq!(state.bottom_render_count, 2);
}

#[test]
fn test_resize_coalesces_over_light() {
    let mut state = make_state();
    simulate_render(&mut state);

    state.scroll(5.0, 0.0); // → Light
    state.resize(1024.0, 768.0, 2.0); // → Full (upgrades)

    let level = simulate_render(&mut state);
    assert_eq!(level, InvalidationLevel::Full);
}

#[test]
fn test_add_pane_coalesces_over_cursor() {
    let mut state = make_state();
    simulate_render(&mut state);

    let cx = state.layout.plot_area.x + 100.0;
    let cy = state.layout.plot_area.y + 100.0;
    state.pointer_move(cx, cy); // → Cursor
    state.add_pane(1.0); // → Full

    let level = simulate_render(&mut state);
    assert_eq!(level, InvalidationLevel::Full);
}

// =============================================================================
// Full lifecycle verification
// =============================================================================

#[test]
fn test_full_lifecycle() {
    let mut state = make_state();

    // Phase 1: Initial render
    let level = simulate_render(&mut state);
    assert_eq!(level, InvalidationLevel::Full);
    assert_eq!(state.bottom_render_count, 1);
    assert_eq!(state.crosshair_render_count, 1);

    // Phase 2: Idle — no renders for 5 frames
    for _ in 0..5 {
        simulate_render(&mut state);
    }
    assert_eq!(state.skipped_render_count, 5);
    assert_eq!(state.bottom_render_count, 1);

    // Phase 3: Mouse enters — cursor only
    let cx = state.layout.plot_area.x + 200.0;
    let cy = state.layout.plot_area.y + 150.0;
    state.pointer_move(cx, cy);
    let level = simulate_render(&mut state);
    assert_eq!(level, InvalidationLevel::Cursor);
    assert_eq!(state.bottom_render_count, 1, "cursor didn't rebuild bottom");
    assert_eq!(state.crosshair_render_count, 2);

    // Phase 4: Scroll — full series redraw
    state.scroll(30.0, 0.0);
    let level = simulate_render(&mut state);
    assert_eq!(level, InvalidationLevel::Light);
    assert_eq!(state.bottom_render_count, 2);

    // Phase 5: Mouse leave → idle
    state.pointer_leave();
    simulate_render(&mut state); // Cursor for hide

    // Phase 6: Idle again
    for _ in 0..3 {
        simulate_render(&mut state);
    }
    let total_skips = state.skipped_render_count;
    assert!(
        total_skips >= 7,
        "should have at least 7 skips (5 initial + some post-leave idle)"
    );

    // Phase 7: Add series → Full
    state
        .series
        .add(lumen_charts::series::Series::line(0, vec![]));
    state.pending_mask.set_global(InvalidationLevel::Full);
    let level = simulate_render(&mut state);
    assert_eq!(level, InvalidationLevel::Full);
    assert_eq!(state.bottom_render_count, 3);
}

#[test]
fn test_keyboard_scroll_invalidation_cycle() {
    let mut state = make_state();
    simulate_render(&mut state); // initial

    // Arrow key → Light → render → idle
    state.key_down(lumen_charts::chart_state::ChartKey::ArrowRight);
    let level = simulate_render(&mut state);
    assert_eq!(level, InvalidationLevel::Light);

    // Idle
    let level = simulate_render(&mut state);
    assert_eq!(level, InvalidationLevel::None);
}

#[test]
fn test_render_counter_accuracy() {
    let mut state = make_state();

    // 1 initial render
    simulate_render(&mut state);

    // 3 idle frames
    for _ in 0..3 {
        simulate_render(&mut state);
    }

    // 2 cursor frames
    for i in 0..2 {
        state.pointer_move(100.0 + i as f32 * 10.0, 200.0);
        simulate_render(&mut state);
    }

    // 1 scroll frame
    state.scroll(10.0, 0.0);
    simulate_render(&mut state);

    // 2 idle frames
    for _ in 0..2 {
        simulate_render(&mut state);
    }

    // Total: 1 + 3 + 2 + 1 + 2 = 9 renders
    assert_eq!(state.bottom_render_count, 2, "1 initial + 1 scroll");
    assert_eq!(
        state.crosshair_render_count, 4,
        "1 initial + 2 cursor + 1 scroll"
    );
    assert_eq!(state.skipped_render_count, 5, "3 + 2 idle frames");
    // bottom + crosshair + skipped should account for all 9 calls:
    // 2 bottom renders, 4 crosshair renders, 5 skipped = but each render call does both bottom+crosshair or just crosshair.
    // Actually: calls that do work = initial(1) + cursor(2) + scroll(1) = 4 non-skipped
    // calls skipped = 5
    // total = 9 ✓
}

// =============================================================================
// Render function verification
// =============================================================================

#[test]
fn test_render_bottom_scene_produces_scene_content() {
    let state = make_state();
    let mut backend = VelloBackend::new();
    render_bottom_scene(&mut backend, &state);
    // Backend scene should have content — verify encoding is non-empty.
    let encoded = backend.scene.encoding();
    assert!(!encoded.is_empty(), "bottom scene should have content");
}

#[test]
fn test_render_crosshair_scene_empty_when_not_visible() {
    let state = make_state();
    let mut backend = VelloBackend::new();
    assert!(!state.crosshair.visible);
    render_crosshair_scene(&mut backend, &state);
    let encoded = backend.scene.encoding();
    assert!(
        encoded.is_empty(),
        "crosshair scene should be empty when not visible"
    );
}

#[test]
fn test_render_crosshair_scene_has_content_when_visible() {
    let mut state = make_state();
    let cx = state.layout.plot_area.x + 100.0;
    let cy = state.layout.plot_area.y + 100.0;
    state.pointer_move(cx, cy);
    assert!(state.crosshair.visible);

    let mut backend = VelloBackend::new();
    render_crosshair_scene(&mut backend, &state);
    let encoded = backend.scene.encoding();
    assert!(
        !encoded.is_empty(),
        "crosshair scene should have content when visible"
    );
}
