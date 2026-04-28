//! PDF rendering via krilla.

use crate::fonts::loader::FontRegistry;
use crate::layout::boxes::BoxContent;
use crate::layout::page::{LayoutPage, PageLayout};

use krilla::color::rgb;
use krilla::font::{Font, GlyphUnits, KrillaGlyph};
use krilla::geom::{Point, Rect};
use krilla::path::{Fill, PathBuilder};
use krilla::{Document, PageSettings};
use skrifa::GlyphId;

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
        0,
        vec![],
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

    let fill = Fill {
        paint: rgb::Color::black().into(),
        ..Default::default()
    };

    for page in pages {
        let page_settings = PageSettings::new(page.width as f32, page.height as f32);
        let mut krilla_page = document.start_page_with(page_settings);
        let mut surface = krilla_page.surface();

        for line in &page.lines {
            let mut current_font: Option<crate::fonts::loader::FontId> = None;
            let mut current_size: f64 = 0.0;
            let mut current_glyphs = Vec::new();
            let mut current_run_start_x = 0.0;

            for box_ in &line.boxes {
                match &box_.content {
                    BoxContent::Glyph {
                        font_id,
                        size_pt,
                        ..
                    } => {
                        let is_new_run = current_font != Some(*font_id) || current_size != *size_pt;

                        if is_new_run && !current_glyphs.is_empty() {
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
                            current_glyphs.clear();
                        }

                        if current_glyphs.is_empty() {
                            current_font = Some(*font_id);
                            current_size = *size_pt;
                            current_run_start_x = box_.x;
                        }

                        let relative_x = box_.x - current_run_start_x;
                        let relative_y = box_.y - line.baseline_y;

                        if let BoxContent::Glyph { glyph_id, x_offset, y_offset, .. } = &box_.content {
                            current_glyphs.push(KrillaGlyph::new(
                                GlyphId::new(*glyph_id as u32),
                                0.0,
                                (relative_x + x_offset) as f32,
                                (relative_y + y_offset) as f32,
                                0.0,
                                0..0,
                            ));
                        }
                    }
                    BoxContent::Rule {
                        width,
                        height,
                        depth,
                    } => {
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
                            current_glyphs.clear();
                            current_font = None;
                        }

                        if let Some(rect) = Rect::from_xywh(
                            box_.x as f32,
                            (2.0 * line.baseline_y - box_.y - height) as f32,
                            *width as f32,
                            (height + depth) as f32,
                        ) {
                            let mut path_builder = PathBuilder::new();
                            path_builder.push_rect(rect);
                            let path = path_builder.finish().unwrap();
                            surface.fill_path(&path, fill.clone());
                        }
                    }
                    BoxContent::Path { points, .. } => {
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
                            current_glyphs.clear();
                            current_font = None;
                        }

                        if points.len() >= 3 {
                            let mut path_builder = PathBuilder::new();
                            let to_pdf = |dx: f64, dy: f64| -> (f32, f32) {
                                (
                                    (box_.x + dx) as f32,
                                    (2.0 * line.baseline_y - box_.y - dy) as f32,
                                )
                            };
                            let (sx, sy) = to_pdf(points[0].0, points[0].1);
                            path_builder.move_to(sx, sy);
                            for (dx, dy) in &points[1..] {
                                let (px, py) = to_pdf(*dx, *dy);
                                path_builder.line_to(px, py);
                            }
                            path_builder.close();
                            if let Some(path) = path_builder.finish() {
                                surface.fill_path(&path, fill.clone());
                            }
                        }
                    }
                    _ => {}
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
