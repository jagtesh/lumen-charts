/// Canonical color module — single source of truth for all color types.
///
/// Contains:
/// - `Color`     — newtype wrapping `[f32; 4]` RGBA
/// - `ColorName` — named base colors used throughout the chart engine
/// - `Palette`   — semantic chart color roles (maps purpose → ColorName)
/// - `GradientStop` — (Color, offset) pair for gradient fills
use serde::{Deserialize, Serialize};

// ---------------------------------------------------------------------------
// Color newtype — single canonical RGBA representation
// ---------------------------------------------------------------------------

/// A color in RGBA format, each component 0.0–1.0.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Color(pub [f32; 4]);

impl Color {
    /// Construct a color from RGBA components.
    pub const fn rgba(r: f32, g: f32, b: f32, a: f32) -> Self {
        Color([r, g, b, a])
    }

    /// Return a copy of this color with a different alpha value.
    pub const fn with_alpha(self, a: f32) -> Self {
        Color([self.0[0], self.0[1], self.0[2], a])
    }
}

impl From<[f32; 4]> for Color {
    fn from(c: [f32; 4]) -> Self {
        Color(c)
    }
}

impl From<Color> for [f32; 4] {
    fn from(c: Color) -> Self {
        c.0
    }
}

impl std::ops::Index<usize> for Color {
    type Output = f32;
    fn index(&self, i: usize) -> &f32 {
        &self.0[i]
    }
}

// ---------------------------------------------------------------------------
// ColorName — named base colors
// ---------------------------------------------------------------------------

/// Named base colors used across the chart engine.
///
/// Each variant maps to a single opaque RGBA value. For transparency
/// variants, call `.color().with_alpha(a)` at the callsite.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ColorName {
    /// Bullish / up elements — teal `(0.15, 0.65, 0.60, 1.0)`
    Teal,
    /// Bearish / down elements — red `(0.94, 0.33, 0.31, 1.0)`
    Red,
    /// Line / area / histogram series default — blue `(0.26, 0.52, 0.96, 1.0)`
    Blue,
    /// General text / labels — slate gray `(0.6, 0.6, 0.7, 1.0)`
    SlateGray,
    /// Chart background — dark charcoal `(0.07, 0.07, 0.10, 1.0)`
    DarkCharcoal,
    /// Layout background — even darker `(0.05, 0.05, 0.07, 1.0)`
    DarkerCharcoal,
    /// Grid lines — dark slate `(0.15, 0.15, 0.20, 1.0)`
    DarkSlate,
    /// Axis lines and tick marks — medium gray `(0.4, 0.4, 0.5, 1.0)`
    MediumGray,
    /// Crosshair lines — light gray `(0.5, 0.5, 0.6, 1.0)`
    LightGray,
    /// Crosshair label background — dark indigo `(0.2, 0.2, 0.3, 1.0)`
    DarkIndigo,
    /// Crosshair info background — midnight blue `(0.12, 0.12, 0.18, 1.0)`
    MidnightBlue,
    /// Pure white — high-contrast text `(1.0, 1.0, 1.0, 1.0)`
    White,
    /// Price line default — crimson `(0.8, 0.2, 0.2, 1.0)`
    Crimson,
    /// Watermark — dark olive `(0.2, 0.2, 0.25, 1.0)`
    DarkOlive,
}

impl ColorName {
    /// Resolve this named color to a concrete `Color`.
    pub const fn color(&self) -> Color {
        match self {
            Self::Teal => Color::rgba(0.15, 0.65, 0.60, 1.0),
            Self::Red => Color::rgba(0.94, 0.33, 0.31, 1.0),
            Self::Blue => Color::rgba(0.26, 0.52, 0.96, 1.0),
            Self::SlateGray => Color::rgba(0.6, 0.6, 0.7, 1.0),
            Self::DarkCharcoal => Color::rgba(0.07, 0.07, 0.10, 1.0),
            Self::DarkerCharcoal => Color::rgba(0.05, 0.05, 0.07, 1.0),
            Self::DarkSlate => Color::rgba(0.15, 0.15, 0.20, 1.0),
            Self::MediumGray => Color::rgba(0.4, 0.4, 0.5, 1.0),
            Self::LightGray => Color::rgba(0.5, 0.5, 0.6, 1.0),
            Self::DarkIndigo => Color::rgba(0.2, 0.2, 0.3, 1.0),
            Self::MidnightBlue => Color::rgba(0.12, 0.12, 0.18, 1.0),
            Self::White => Color::rgba(1.0, 1.0, 1.0, 1.0),
            Self::Crimson => Color::rgba(0.8, 0.2, 0.2, 1.0),
            Self::DarkOlive => Color::rgba(0.2, 0.2, 0.25, 1.0),
        }
    }
}

// ---------------------------------------------------------------------------
// Palette — semantic chart color roles
// ---------------------------------------------------------------------------

/// Named color palette for standard chart elements.
///
/// Each variant maps a *purpose* (e.g. background, grid) to a `ColorName`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Palette {
    /// Main chart background
    Background,
    /// Grid lines
    Grid,
    /// Axis lines and tick marks
    Axis,
    /// Bullish (up) elements
    Bull,
    /// Bearish (down) elements
    Bear,
    /// General text / labels
    Text,
    /// Crosshair lines
    Crosshair,
    /// Crosshair price-label background
    CrosshairLabelBg,
    /// Crosshair OHLC-info background
    CrosshairInfoBg,
    /// Pure white (for text on colored backgrounds)
    White,
}

impl Palette {
    /// Resolve this palette entry to a concrete `Color`.
    pub const fn color(&self) -> Color {
        match self {
            Self::Background => ColorName::DarkCharcoal.color(),
            Self::Grid => ColorName::DarkSlate.color(),
            Self::Axis => ColorName::MediumGray.color(),
            Self::Bull => ColorName::Teal.color(),
            Self::Bear => ColorName::Red.color(),
            Self::Text => ColorName::SlateGray.color(),
            Self::Crosshair => ColorName::LightGray.color().with_alpha(0.8),
            Self::CrosshairLabelBg => ColorName::DarkIndigo.color().with_alpha(0.9),
            Self::CrosshairInfoBg => ColorName::MidnightBlue.color().with_alpha(0.9),
            Self::White => ColorName::White.color(),
        }
    }
}

/// A gradient stop: (color, offset). Offset is 0.0 (start) to 1.0 (end).
pub type GradientStop = (Color, f32);
