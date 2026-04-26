//! PDF rendering via krilla.

use crate::fonts::loader::FontRegistry;
use crate::layout::boxes::BoxContent;
use crate::layout::page::{LayoutPage, PageLayout};

use krilla::color::rgb;
use krilla::font::{Font, GlyphUnits, KrillaGlyph};
use krilla::path::Fill;
use krilla::{Document, PageSettings};
use skrifa::GlyphId;
use krilla::geom::Point;

/// Helper to get a Krilla Font from our FontRegistry.
fn get_krilla_font(
    font_id: crate::fonts::loader::FontId,
    registry: &FontRegistry,
    cache: &mut std::collections::HashMap<u8, Font>,
) -> miette::Result<Font> {
    if let Some(font) = cache.get(&font_id.0) {
        return Ok(font.clone());
    }

    let loaded_font = registry.get(font_id);
    let krilla_font = Font::new(
        loaded_font.data.clone(),
        0, // assume face index 0
        vec![], // features
    )
    .ok_or_else(|| miette::miette!("Failed to load PDF font"))?;

    cache.insert(font_id.0, krilla_font.clone());
    Ok(krilla_font)
}

/// Render laid-out pages to a PDF byte vector.
pub fn render_to_pdf(
    pages: &[LayoutPage],
    fonts: &FontRegistry,
    _layout: &PageLayout,
) -> miette::Result<Vec<u8>> {
    let mut document = Document::new();
    let mut font_cache = std::collections::HashMap::<u8, Font>::new();

    // Default black paint for text
    let fill = Fill {
        paint: rgb::Color::black().into(),
        ..Default::default()
    };

    for page in pages {
        let page_settings = PageSettings::new(page.width as f32, page.height as f32);
        let mut krilla_page = document.start_page_with(page_settings);
        let mut surface = krilla_page.surface();

        for line in &page.lines {
            // Because krilla clusters glyphs by font and style, we should group consecutive
            // glyphs of the same font and size.
            let mut current_font: Option<crate::fonts::loader::FontId> = None;
            let mut current_size: f64 = 0.0;
            let mut current_glyphs = Vec::new();
            let mut current_run_start_x = 0.0;

            for box_ in &line.boxes {
                match &box_.content {
                    BoxContent::Glyph {
                        font_id,
                        glyph_id,
                        size_pt,
                        width,
                        ..
                    } => {
                        let is_new_run = current_font != Some(*font_id) || current_size != *size_pt;

                        if is_new_run && !current_glyphs.is_empty() {
                            // Flush current run
                            let k_font = get_krilla_font(current_font.unwrap(), fonts, &mut font_cache)?;
                            surface.fill_glyphs(
                                Point::from_xy(current_run_start_x as f32, line.baseline_y as f32),
                                fill.clone(),
                                &current_glyphs,
                                k_font,
                                "", // Text mapping empty for now
                                current_size as f32,
                                GlyphUnits::UserSpace,
                                false,
                            );

                            current_glyphs.clear();
                        }

                        if current_glyphs.is_empty() {
                            current_font = Some(*font_id);
                            current_size = *size_pt;
                            current_run_start_x = box_.x;
                        }

                        let relative_x = box_.x - current_run_start_x;

                        current_glyphs.push(KrillaGlyph::new(
                            GlyphId::new(*glyph_id as u32),
                            *width as f32,
                            relative_x as f32,
                            0.0, // box_.y relative to baseline is usually 0 unless doing sub/superscripts
                            0.0,
                            0..0, // empty text range
                        ));
                    }
                    _ => {
                        // Image, rule, link processing stub.
                    }
                }
            }

            if !current_glyphs.is_empty() {
                let k_font = get_krilla_font(current_font.unwrap(), fonts, &mut font_cache)?;
                surface.fill_glyphs(
                    Point::from_xy(current_run_start_x as f32, line.baseline_y as f32),
                    fill.clone(),
                    &current_glyphs,
                    k_font,
                    "",
                    current_size as f32,
                    GlyphUnits::UserSpace,
                    false,
                );
            }
        }

        surface.finish();
        krilla_page.finish();
    }

    document
        .finish()
        .map_err(|e| miette::miette!("Failed to finalize PDF: {:?}", e))
}
