//! Text shaping via rustybuzz.
//!
//! Takes a string and font, runs OpenType shaping (ligatures, kerning, etc.),
//! and returns positioned glyphs scaled to the requested point size.

use crate::fonts::loader::LoadedFont;

/// A shaped glyph with position information, all in points.
#[derive(Debug, Clone)]
pub struct ShapedGlyph {
    /// Glyph ID in the font.
    pub glyph_id: u16,
    /// Horizontal advance in points.
    pub x_advance: f64,
    /// Vertical advance in points.
    pub y_advance: f64,
    /// Horizontal offset from current position.
    pub x_offset: f64,
    /// Vertical offset from current position.
    pub y_offset: f64,
    /// Byte offset into the original text (cluster).
    pub cluster: u32,
}

/// Shape a text string using rustybuzz, returning positioned glyphs in points.
///
/// 1. Creates a `rustybuzz::UnicodeBuffer` and pushes the text.
/// 2. Sets direction (LTR for Latin).
/// 3. Calls `rustybuzz::shape()`.
/// 4. Converts `GlyphInfo` + `GlyphPosition` to [`ShapedGlyph`].
/// 5. Scales by `size_pt / units_per_em`.
pub fn shape_text(
    font: &LoadedFont,
    text: &str,
    size_pt: f64,
    direction: rustybuzz::Direction,
) -> Vec<ShapedGlyph> {
    let mut buffer = rustybuzz::UnicodeBuffer::new();
    buffer.push_str(text);
    buffer.set_direction(direction);

    let glyph_buffer = rustybuzz::shape(&font.face, &[], buffer);

    let scale = size_pt / font.units_per_em as f64;
    let infos = glyph_buffer.glyph_infos();
    let positions = glyph_buffer.glyph_positions();

    infos
        .iter()
        .zip(positions.iter())
        .map(|(info, pos)| ShapedGlyph {
            glyph_id: info.glyph_id as u16,
            x_advance: pos.x_advance as f64 * scale,
            y_advance: pos.y_advance as f64 * scale,
            x_offset: pos.x_offset as f64 * scale,
            y_offset: pos.y_offset as f64 * scale,
            cluster: info.cluster,
        })
        .collect()
}

/// Shape a single character, returning its glyph info.
pub fn shape_char(font: &LoadedFont, ch: char, size_pt: f64) -> Option<ShapedGlyph> {
    let mut s = String::new();
    s.push(ch);
    let glyphs = shape_text(font, &s, size_pt, rustybuzz::Direction::LeftToRight);
    glyphs.into_iter().next()
}

/// Measure the total width of shaped text in points.
pub fn measure_text_width(font: &LoadedFont, text: &str, size_pt: f64) -> f64 {
    let glyphs = shape_text(font, text, size_pt, rustybuzz::Direction::LeftToRight);
    glyphs.iter().map(|g| g.x_advance).sum()
}
