//! Glyph metrics from font data via skrifa.
//!
//! All metrics are returned in points, scaled from the font's design units
//! by `(value / units_per_em) * size_pt`.

use crate::fonts::loader::LoadedFont;
use skrifa::instance::Size;
use skrifa::MetadataProvider;

/// Metrics for a single glyph, scaled to the requested point size.
#[derive(Debug, Clone)]
pub struct GlyphMetrics {
    /// Advance width in points.
    pub advance_width: f64,
    /// Left side bearing in points.
    pub lsb: f64,
    /// Minimum x coordinate of the glyph bounding box.
    pub x_min: f64,
    /// Minimum y coordinate (below baseline, negative = descender).
    pub y_min: f64,
    /// Maximum x coordinate.
    pub x_max: f64,
    /// Maximum y coordinate (above baseline).
    pub y_max: f64,
}

/// Retrieve metrics for a single glyph at the given point size.
pub fn glyph_metrics(
    font: &LoadedFont,
    glyph_id: skrifa::GlyphId,
    size_pt: f64,
) -> Option<GlyphMetrics> {
    let font_data = font.data.as_slice();
    let font_ref = skrifa::FontRef::new(font_data).ok()?;
    let size = Size::new(size_pt as f32);
    let glyph_metrics = font_ref.glyph_metrics(size, skrifa::instance::LocationRef::default());

    let advance_width = glyph_metrics.advance_width(glyph_id).unwrap_or(0.0) as f64;
    let lsb = glyph_metrics.left_side_bearing(glyph_id).unwrap_or(0.0) as f64;

    // Get bounding box if available
    let bounds = glyph_metrics.bounds(glyph_id);
    let (x_min, y_min, x_max, y_max) = if let Some(b) = bounds {
        (
            b.x_min as f64,
            b.y_min as f64,
            b.x_max as f64,
            b.y_max as f64,
        )
    } else {
        (0.0, 0.0, advance_width, size_pt * 0.7) // fallback
    };

    Some(GlyphMetrics {
        advance_width,
        lsb,
        x_min,
        y_min,
        x_max,
        y_max,
    })
}

/// Look up a glyph ID for a character in the given font.
pub fn char_to_glyph(font: &LoadedFont, ch: char) -> Option<skrifa::GlyphId> {
    let font_data = font.data.as_slice();
    let font_ref = skrifa::FontRef::new(font_data).ok()?;
    let charmap = font_ref.charmap();
    charmap.map(ch)
}

/// Get the font's ascender in points.
pub fn ascender_pt(font: &LoadedFont, size_pt: f64) -> f64 {
    let scale = size_pt / font.units_per_em as f64;
    let font_data = font.data.as_slice();
    if let Ok(font_ref) = skrifa::FontRef::new(font_data) {
        let metrics = font_ref.metrics(
            Size::new(size_pt as f32),
            skrifa::instance::LocationRef::default(),
        );
        metrics.ascent as f64
    } else {
        size_pt * 0.8 * scale.signum() // fallback
    }
}

/// Get the font's descender in points (typically negative).
pub fn descender_pt(font: &LoadedFont, size_pt: f64) -> f64 {
    let font_data = font.data.as_slice();
    if let Ok(font_ref) = skrifa::FontRef::new(font_data) {
        let metrics = font_ref.metrics(
            Size::new(size_pt as f32),
            skrifa::instance::LocationRef::default(),
        );
        metrics.descent as f64
    } else {
        -size_pt * 0.2
    }
}

/// Get the space width for a font at a given size.
pub fn space_width_pt(font: &LoadedFont, size_pt: f64) -> f64 {
    if let Some(gid) = char_to_glyph(font, ' ') {
        if let Some(metrics) = glyph_metrics(font, gid, size_pt) {
            return metrics.advance_width;
        }
    }
    // Fallback: roughly 1/3 em
    size_pt / 3.0
}
