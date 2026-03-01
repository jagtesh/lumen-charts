use std::sync::Arc;

use vello::kurbo::Affine;
use vello::peniko::{Blob, Brush, Color, Fill, Font};
use vello::Scene;
use vello_encoding::Glyph;

use skrifa::instance::{LocationRef, Size};
use skrifa::MetadataProvider;

/// Embedded Roboto Mono font data (variable weight, Apache 2.0 license)
static FONT_DATA: &[u8] = include_bytes!("../fonts/RobotoMono.ttf");

/// Get the embedded chart font
pub fn chart_font() -> Font {
    Font::new(Blob::new(Arc::new(FONT_DATA.to_vec())), 0)
}

/// Draw a text string at the given position using Vello's GPU glyph rendering.
///
/// `x`, `y` are in logical coordinates (pre-scale). `y` is the baseline.
pub fn draw_text(
    scene: &mut Scene,
    font: &Font,
    text: &str,
    x: f64,
    y: f64,
    font_size: f32,
    color: Color,
    transform: Affine,
) {
    if text.is_empty() {
        return;
    }

    // Use skrifa to resolve character -> glyph ID and compute advances
    let font_ref = match skrifa::FontRef::from_index(font.data.as_ref(), font.index) {
        Ok(f) => f,
        Err(_) => return,
    };

    let charmap = font_ref.charmap();
    let glyph_metrics = font_ref.glyph_metrics(Size::new(font_size), LocationRef::default());

    let mut glyphs = Vec::with_capacity(text.len());
    let mut cursor_x = x as f32;

    for ch in text.chars() {
        let glyph_id = charmap.map(ch as u32).unwrap_or_default();

        glyphs.push(Glyph {
            id: glyph_id.to_u32(),
            x: cursor_x,
            y: y as f32,
        });

        // Advance the cursor
        let advance = glyph_metrics
            .advance_width(glyph_id)
            .unwrap_or(font_size * 0.6);
        cursor_x += advance;
    }

    scene
        .draw_glyphs(font)
        .font_size(font_size)
        .transform(transform)
        .brush(&Brush::Solid(color))
        .draw(Fill::NonZero, glyphs.into_iter());
}

/// Measure the width of a text string (in logical pixels).
pub fn measure_text(font: &Font, text: &str, font_size: f32) -> f32 {
    if text.is_empty() {
        return 0.0;
    }

    let font_ref = match skrifa::FontRef::from_index(font.data.as_ref(), font.index) {
        Ok(f) => f,
        Err(_) => return text.len() as f32 * font_size * 0.6,
    };

    let charmap = font_ref.charmap();
    let glyph_metrics = font_ref.glyph_metrics(Size::new(font_size), LocationRef::default());

    let mut width = 0.0f32;
    for ch in text.chars() {
        let glyph_id = charmap.map(ch as u32).unwrap_or_default();
        let advance = glyph_metrics
            .advance_width(glyph_id)
            .unwrap_or(font_size * 0.6);
        width += advance;
    }
    width
}
