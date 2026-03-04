use std::sync::Arc;

use vello::kurbo::Affine;
use vello::peniko::{Blob, Brush, Color, Fill, Font};
use vello::Scene;
use vello_encoding::Glyph;

use skrifa::instance::{LocationRef, Size};
use skrifa::MetadataProvider;

/// Embedded Inter font data (proportional sans-serif, SIL Open Font License)
static FONT_DATA: &[u8] = include_bytes!("../fonts/Inter.ttf");

/// Get the embedded chart font (for passing to Vello draw_glyphs)
pub fn chart_font() -> Font {
    Font::new(Blob::new(Arc::new(FONT_DATA.to_vec())), 0)
}

/// Get a skrifa FontRef for glyph metrics (uses the raw embedded bytes directly)
fn font_ref() -> skrifa::FontRef<'static> {
    // Use the raw static bytes — avoids Blob/Arc indirection issues
    skrifa::FontRef::from_index(FONT_DATA, 0).expect("Embedded font should be valid")
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

    let fref = font_ref();
    let charmap = fref.charmap();
    let glyph_metrics = fref.glyph_metrics(Size::new(font_size), LocationRef::default());

    let mut glyphs = Vec::with_capacity(text.len());
    let mut cursor_x = x as f32;

    for ch in text.chars() {
        let glyph_id = charmap.map(ch as u32).unwrap_or_default();

        glyphs.push(Glyph {
            id: glyph_id.to_u32(),
            x: cursor_x,
            y: y as f32,
        });

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
pub fn measure_text(_font: &Font, text: &str, font_size: f32) -> f32 {
    if text.is_empty() {
        return 0.0;
    }

    let fref = font_ref();
    let charmap = fref.charmap();
    let glyph_metrics = fref.glyph_metrics(Size::new(font_size), LocationRef::default());

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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_font_loads() {
        let font = chart_font();
        assert!(!font.data.is_empty());
    }

    #[test]
    fn test_font_ref_parses() {
        let fref = font_ref();
        let charmap = fref.charmap();
        let glyph = charmap.map('A' as u32);
        assert!(glyph.is_some(), "Should find glyph for 'A'");
    }

    #[test]
    fn test_glyph_resolution() {
        let fref = font_ref();
        let charmap = fref.charmap();

        for ch in "0123456789.,-:ABCabc".chars() {
            let glyph = charmap.map(ch as u32);
            assert!(glyph.is_some(), "Glyph not found for char '{}'", ch);
            assert!(glyph.unwrap().to_u32() > 0, "Glyph ID 0 for char '{}'", ch);
        }
    }

    #[test]
    fn test_measure_text_positive() {
        let font = chart_font();
        let width = measure_text(&font, "123.45", 11.0);
        assert!(width > 0.0, "Text width should be positive, got {}", width);
        assert!(width > 30.0, "Text width seems too small: {}", width);
        assert!(width < 200.0, "Text width seems too large: {}", width);
    }

    #[test]
    fn test_advance_widths() {
        let fref = font_ref();
        let charmap = fref.charmap();
        let glyph_metrics = fref.glyph_metrics(Size::new(11.0), LocationRef::default());

        for ch in "0123456789".chars() {
            let glyph_id = charmap.map(ch as u32).unwrap();
            let advance = glyph_metrics.advance_width(glyph_id);
            assert!(advance.is_some(), "No advance width for char '{}'", ch);
            let advance = advance.unwrap();
            assert!(
                advance > 0.0 && advance < 50.0,
                "Bad advance {} for '{}'",
                advance,
                ch
            );
        }
    }
}
