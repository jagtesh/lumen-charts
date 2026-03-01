//! Invalidation system for efficient chart rendering.
//!
//! Tracks what needs to be redrawn and at what granularity.
//! Mirrors the LWC invalidation architecture with 4 levels:
//! None → Cursor → Light → Full.

use std::collections::HashMap;

/// Invalidation levels, ordered from cheapest to most expensive.
///
/// Higher levels subsume lower ones: a `Light` invalidation includes
/// everything that `Cursor` would redraw, plus more.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(u8)]
pub enum InvalidationLevel {
    /// No redraw needed.
    None = 0,
    /// Crosshair overlay only (top layer).
    /// Triggered by: mouse move, cursor update.
    Cursor = 1,
    /// Series + axes + grid + crosshair.
    /// Triggered by: data update, scroll, zoom, price scale change.
    Light = 2,
    /// Full GUI rebuild + everything in Light.
    /// Triggered by: add/remove pane, add/remove series, options change, resize.
    Full = 3,
}

impl Default for InvalidationLevel {
    fn default() -> Self {
        InvalidationLevel::None
    }
}

impl InvalidationLevel {
    /// Returns the more expensive of two levels.
    pub fn max(self, other: Self) -> Self {
        if self >= other {
            self
        } else {
            other
        }
    }

    /// Whether this level requires a bottom-scene rebuild.
    pub fn needs_bottom_scene(&self) -> bool {
        *self >= InvalidationLevel::Light
    }

    /// Whether this level requires a layout rebuild.
    pub fn needs_layout_rebuild(&self) -> bool {
        *self >= InvalidationLevel::Full
    }
}

/// Coalesced invalidation mask with per-pane granularity.
///
/// Multiple invalidations between render frames are merged by taking the
/// maximum level at each scope (global and per-pane).
#[derive(Debug, Clone)]
pub struct InvalidateMask {
    /// Global invalidation level (applies to all panes).
    global_level: InvalidationLevel,
    /// Per-pane overrides. A pane's effective level is max(global, pane-specific).
    pane_levels: HashMap<u32, InvalidationLevel>,
}

impl Default for InvalidateMask {
    fn default() -> Self {
        Self {
            global_level: InvalidationLevel::None,
            pane_levels: HashMap::new(),
        }
    }
}

impl InvalidateMask {
    // -- Factory methods --

    pub fn none() -> Self {
        Self::default()
    }

    pub fn cursor() -> Self {
        Self {
            global_level: InvalidationLevel::Cursor,
            pane_levels: HashMap::new(),
        }
    }

    pub fn light() -> Self {
        Self {
            global_level: InvalidationLevel::Light,
            pane_levels: HashMap::new(),
        }
    }

    pub fn full() -> Self {
        Self {
            global_level: InvalidationLevel::Full,
            pane_levels: HashMap::new(),
        }
    }

    // -- Accessors --

    /// The global invalidation level.
    pub fn global_level(&self) -> InvalidationLevel {
        self.global_level
    }

    /// Effective invalidation level for a specific pane.
    /// Returns max(global_level, pane-specific level).
    pub fn level_for_pane(&self, pane_id: u32) -> InvalidationLevel {
        let pane_level = self
            .pane_levels
            .get(&pane_id)
            .copied()
            .unwrap_or(InvalidationLevel::None);
        self.global_level.max(pane_level)
    }

    /// Whether anything needs redrawing at all.
    pub fn needs_redraw(&self) -> bool {
        self.global_level > InvalidationLevel::None
            || self
                .pane_levels
                .values()
                .any(|l| *l > InvalidationLevel::None)
    }

    // -- Mutators --

    /// Set the global invalidation level (takes max with current).
    pub fn set_global(&mut self, level: InvalidationLevel) {
        self.global_level = self.global_level.max(level);
    }

    /// Invalidate a specific pane at a specific level.
    pub fn invalidate_pane(&mut self, pane_id: u32, level: InvalidationLevel) {
        let entry = self
            .pane_levels
            .entry(pane_id)
            .or_insert(InvalidationLevel::None);
        *entry = (*entry).max(level);
    }

    /// Merge another mask into this one. Takes the max at every scope.
    pub fn merge(&mut self, other: &InvalidateMask) {
        self.global_level = self.global_level.max(other.global_level);
        for (&pane_id, &level) in &other.pane_levels {
            self.invalidate_pane(pane_id, level);
        }
    }

    /// Reset the mask to None (call after rendering).
    pub fn reset(&mut self) {
        self.global_level = InvalidationLevel::None;
        self.pane_levels.clear();
    }
}

// =============================================================================
// Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    // -- Category 1: InvalidationLevel ordering & semantics --

    #[test]
    fn test_invalidation_level_ordering() {
        assert!(InvalidationLevel::None < InvalidationLevel::Cursor);
        assert!(InvalidationLevel::Cursor < InvalidationLevel::Light);
        assert!(InvalidationLevel::Light < InvalidationLevel::Full);
    }

    #[test]
    fn test_invalidation_level_max() {
        assert_eq!(
            InvalidationLevel::Cursor.max(InvalidationLevel::Light),
            InvalidationLevel::Light
        );
        assert_eq!(
            InvalidationLevel::Full.max(InvalidationLevel::Cursor),
            InvalidationLevel::Full
        );
        assert_eq!(
            InvalidationLevel::None.max(InvalidationLevel::None),
            InvalidationLevel::None
        );
    }

    #[test]
    fn test_needs_bottom_scene() {
        assert!(!InvalidationLevel::None.needs_bottom_scene());
        assert!(!InvalidationLevel::Cursor.needs_bottom_scene());
        assert!(InvalidationLevel::Light.needs_bottom_scene());
        assert!(InvalidationLevel::Full.needs_bottom_scene());
    }

    #[test]
    fn test_needs_layout_rebuild() {
        assert!(!InvalidationLevel::None.needs_layout_rebuild());
        assert!(!InvalidationLevel::Cursor.needs_layout_rebuild());
        assert!(!InvalidationLevel::Light.needs_layout_rebuild());
        assert!(InvalidationLevel::Full.needs_layout_rebuild());
    }

    #[test]
    fn test_default_is_none() {
        assert_eq!(InvalidationLevel::default(), InvalidationLevel::None);
        let mask = InvalidateMask::default();
        assert_eq!(mask.global_level(), InvalidationLevel::None);
        assert!(!mask.needs_redraw());
    }

    // -- Category 2: Mask construction & merging --

    #[test]
    fn test_mask_factory_methods() {
        assert_eq!(
            InvalidateMask::none().global_level(),
            InvalidationLevel::None
        );
        assert_eq!(
            InvalidateMask::cursor().global_level(),
            InvalidationLevel::Cursor
        );
        assert_eq!(
            InvalidateMask::light().global_level(),
            InvalidationLevel::Light
        );
        assert_eq!(
            InvalidateMask::full().global_level(),
            InvalidationLevel::Full
        );
    }

    #[test]
    fn test_mask_merge_takes_max_level() {
        let mut a = InvalidateMask::cursor();
        let b = InvalidateMask::light();
        a.merge(&b);
        assert_eq!(a.global_level(), InvalidationLevel::Light);
    }

    #[test]
    fn test_mask_merge_is_commutative() {
        let mut a = InvalidateMask::light();
        let b = InvalidateMask::cursor();
        a.merge(&b);
        assert_eq!(a.global_level(), InvalidationLevel::Light);
    }

    #[test]
    fn test_mask_per_pane_independence() {
        let mut mask = InvalidateMask::none();
        mask.invalidate_pane(0, InvalidationLevel::Light);
        mask.invalidate_pane(1, InvalidationLevel::Cursor);

        assert_eq!(mask.level_for_pane(0), InvalidationLevel::Light);
        assert_eq!(mask.level_for_pane(1), InvalidationLevel::Cursor);
        assert_eq!(mask.level_for_pane(2), InvalidationLevel::None); // unset pane
    }

    #[test]
    fn test_mask_global_overrides_pane() {
        let mut mask = InvalidateMask::light();
        mask.invalidate_pane(0, InvalidationLevel::Cursor);
        // Global Light > Pane Cursor
        assert_eq!(mask.level_for_pane(0), InvalidationLevel::Light);
    }

    #[test]
    fn test_mask_pane_overrides_lower_global() {
        let mut mask = InvalidateMask::cursor();
        mask.invalidate_pane(0, InvalidationLevel::Full);
        // Pane Full > Global Cursor
        assert_eq!(mask.level_for_pane(0), InvalidationLevel::Full);
        // Other pane only gets global
        assert_eq!(mask.level_for_pane(1), InvalidationLevel::Cursor);
    }

    // -- Category 3: Mask coalescing --

    #[test]
    fn test_multiple_merges_coalesce() {
        let mut mask = InvalidateMask::none();
        mask.merge(&InvalidateMask::cursor());
        mask.merge(&InvalidateMask::cursor());
        mask.merge(&InvalidateMask::cursor());
        assert_eq!(mask.global_level(), InvalidationLevel::Cursor);

        mask.merge(&InvalidateMask::light());
        assert_eq!(mask.global_level(), InvalidationLevel::Light);
    }

    #[test]
    fn test_full_absorbs_all() {
        let mut mask = InvalidateMask::full();
        mask.merge(&InvalidateMask::cursor());
        mask.merge(&InvalidateMask::light());
        assert_eq!(mask.global_level(), InvalidationLevel::Full);
    }

    #[test]
    fn test_none_mask_is_identity() {
        let mut mask = InvalidateMask::light();
        mask.merge(&InvalidateMask::none());
        assert_eq!(mask.global_level(), InvalidationLevel::Light);
    }

    #[test]
    fn test_mask_reset() {
        let mut mask = InvalidateMask::full();
        mask.invalidate_pane(0, InvalidationLevel::Light);
        mask.reset();
        assert_eq!(mask.global_level(), InvalidationLevel::None);
        assert_eq!(mask.level_for_pane(0), InvalidationLevel::None);
        assert!(!mask.needs_redraw());
    }

    #[test]
    fn test_needs_redraw() {
        assert!(!InvalidateMask::none().needs_redraw());
        assert!(InvalidateMask::cursor().needs_redraw());
        assert!(InvalidateMask::light().needs_redraw());
        assert!(InvalidateMask::full().needs_redraw());

        // Pane-level only
        let mut mask = InvalidateMask::none();
        assert!(!mask.needs_redraw());
        mask.invalidate_pane(0, InvalidationLevel::Cursor);
        assert!(mask.needs_redraw());
    }

    #[test]
    fn test_merge_pane_levels() {
        let mut a = InvalidateMask::none();
        a.invalidate_pane(0, InvalidationLevel::Cursor);

        let mut b = InvalidateMask::none();
        b.invalidate_pane(0, InvalidationLevel::Light);
        b.invalidate_pane(1, InvalidationLevel::Cursor);

        a.merge(&b);
        assert_eq!(a.level_for_pane(0), InvalidationLevel::Light); // max(Cursor, Light)
        assert_eq!(a.level_for_pane(1), InvalidationLevel::Cursor);
    }

    #[test]
    fn test_set_global_takes_max() {
        let mut mask = InvalidateMask::cursor();
        mask.set_global(InvalidationLevel::Light);
        assert_eq!(mask.global_level(), InvalidationLevel::Light);
        // Setting a lower level doesn't downgrade
        mask.set_global(InvalidationLevel::Cursor);
        assert_eq!(mask.global_level(), InvalidationLevel::Light);
    }
}
