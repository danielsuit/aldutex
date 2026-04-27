//! Page layout: margins, page geometry, and document-level layout.

use crate::ast;
use crate::fonts::loader::{FontId, FontRegistry};
use crate::layout::boxes::{BoxContent, LayoutBox, LayoutLine};
use crate::layout::paragraph::{break_paragraph, items_to_lines, Item};

/// Page geometry and margins (all values in points).
#[derive(Debug, Clone)]
pub struct PageLayout {
    pub width_pt: f64,
    pub height_pt: f64,
    pub margin_top: f64,
    pub margin_bot: f64,
    pub margin_left: f64,
    pub margin_right: f64,
}

impl PageLayout {
    pub fn text_width(&self) -> f64 {
        self.width_pt - self.margin_left - self.margin_right
    }

    pub fn text_height(&self) -> f64 {
        self.height_pt - self.margin_top - self.margin_bot
    }

    pub fn a4_default() -> Self {
        Self {
            width_pt: 595.0,
            height_pt: 842.0,
            margin_top: 72.0,
            margin_bot: 72.0,
            margin_left: 72.0,
            margin_right: 72.0,
        }
    }

    pub fn letter_default() -> Self {
        Self {
            width_pt: 612.0,
            height_pt: 792.0,
            margin_top: 72.0,
            margin_bot: 72.0,
            margin_left: 72.0,
            margin_right: 72.0,
        }
    }

    pub fn from_document_class(dc: &ast::DocumentClass) -> Self {
        let has_a4 = dc.options.iter().any(|o| o == "a4paper");
        if has_a4 {
            Self::a4_default()
        } else {
            Self::letter_default()
        }
    }
}

/// A complete laid-out page.
#[derive(Debug, Clone)]
pub struct LayoutPage {
    pub width: f64,
    pub height: f64,
    pub lines: Vec<LayoutLine>,
    pub footnotes: Vec<Vec<LayoutLine>>,
}

/// Helper context for turning AST inlines into Knuth-Plass items.
struct LayoutContext<'a> {
    fonts: &'a FontRegistry,
    size_pt: f64,
}

/// Build Knuth-Plass items from AST inlines.
fn build_paragraph_items(
    ctx: &LayoutContext,
    inlines: &[ast::Inline],
    font_id: FontId,
) -> Vec<Item> {
    let mut items = Vec::new();

    fn process_inlines(
        ctx: &LayoutContext,
        inlines: &[ast::Inline],
        font_id: FontId,
        items: &mut Vec<Item>,
    ) {
        let font = ctx.fonts.get(font_id);
        let space_width = crate::fonts::metrics::space_width_pt(font, ctx.size_pt);
        let ascender = crate::fonts::metrics::ascender_pt(font, ctx.size_pt);
        let descender = crate::fonts::metrics::descender_pt(font, ctx.size_pt);

        for inline in inlines {
            match inline {
                ast::Inline::Text { content, .. } => {
                    let mut words = content.split(' ').peekable();
                    while let Some(word) = words.next() {
                        if !word.is_empty() {
                            let shaped = crate::fonts::shaper::shape_text(
                                font,
                                word,
                                ctx.size_pt,
                                rustybuzz::Direction::LeftToRight,
                            );

                            let mut total_width = 0.0;
                            let mut current_x = 0.0;
                            let mut boxes = Vec::new();

                            for g in shaped {
                                boxes.push(LayoutBox {
                                    x: current_x + g.x_offset,
                                    y: g.y_offset,
                                    content: BoxContent::Glyph {
                                        font_id,
                                        glyph_id: g.glyph_id,
                                        size_pt: ctx.size_pt,
                                        width: g.x_advance,
                                        x_offset: g.x_offset,
                                        y_offset: g.y_offset,
                                        height: ascender,
                                        depth: descender,
                                    },
                                });
                                current_x += g.x_advance;
                                total_width += g.x_advance;
                            }

                            items.push(Item::Box {
                                width: total_width,
                                content: boxes,
                            });
                        }

                        if words.peek().is_some() {
                            // There was a space
                            items.push(Item::Glue {
                                width: space_width,
                                stretch: space_width / 2.0,
                                shrink: space_width / 3.0,
                            });
                        }
                    }
                }
                ast::Inline::Bold { content, .. } => {
                    process_inlines(ctx, content, FontRegistry::bold(), items);
                }
                ast::Inline::Italic { content, .. } => {
                    process_inlines(ctx, content, FontRegistry::italic(), items);
                }
                ast::Inline::BoldItalic { content, .. } => {
                    process_inlines(ctx, content, FontRegistry::bolditalic(), items);
                }
                ast::Inline::Monospace { content, .. } => {
                    process_inlines(ctx, content, FontRegistry::mono(), items);
                }
                ast::Inline::LineBreak { .. } => {
                    items.push(Item::Penalty {
                        width: 0.0,
                        penalty: -10000.0,
                        flagged: false,
                    });
                }
                ast::Inline::NonBreakingSpace { .. } => {
                    items.push(Item::Penalty {
                        width: 0.0,
                        penalty: 10000.0,
                        flagged: false,
                    });
                    items.push(Item::Glue {
                        width: space_width,
                        stretch: space_width / 2.0,
                        shrink: space_width / 3.0,
                    });
                }
                ast::Inline::Math { node, .. } => {
                    let math_layout = super::math::layout_math(
                        node,
                        ctx.fonts,
                        ctx.size_pt,
                        super::math::LayoutStyle::Text,
                    );
                    items.push(Item::Box {
                        width: math_layout.width,
                        content: math_layout.boxes,
                    });
                }
                _ => {} // Remaining inlines simplified for Stage 6
            }
        }
    }

    process_inlines(ctx, inlines, font_id, &mut items);

    // End paragraph forced break
    items.push(Item::Glue {
        width: 0.0,
        stretch: 10000.0, // Infinite stretch
        shrink: 0.0,
    });
    items.push(Item::Penalty {
        width: 0.0,
        penalty: -10000.0,
        flagged: true,
    });

    items
}

/// Lay out an entire document into pages.
pub fn layout_document(
    doc: &ast::Document,
    fonts: &FontRegistry,
    layout: &PageLayout,
) -> Vec<LayoutPage> {
    let mut pages = Vec::new();
    let mut current_page_lines = Vec::new();
    let mut current_y = layout.margin_top;

    layout_blocks(
        &doc.body,
        fonts,
        layout,
        &mut current_y,
        &mut current_page_lines,
        &mut pages,
    );

    // Flush final page
    if !current_page_lines.is_empty() || pages.is_empty() {
        pages.push(LayoutPage {
            width: layout.width_pt,
            height: layout.height_pt,
            lines: current_page_lines,
            footnotes: Vec::new(),
        });
    }

    pages
}

fn layout_blocks(
    blocks: &[ast::Block],
    fonts: &FontRegistry,
    layout: &PageLayout,
    current_y: &mut f64,
    current_page_lines: &mut Vec<LayoutLine>,
    pages: &mut Vec<LayoutPage>,
) {
    let text_width = layout.text_width();
    let max_y = layout.height_pt - layout.margin_bot;

    for block in blocks {
        let mut block_lines = Vec::new();

        match block {
            ast::Block::Paragraph { inlines, .. } => {
                let ctx = LayoutContext {
                    fonts,
                    size_pt: 10.0,
                };
                let items = build_paragraph_items(&ctx, inlines, FontRegistry::regular());
                let breaks = break_paragraph(&items, text_width, 10.0, 50.0);
                block_lines = items_to_lines(&items, &breaks, text_width);
            }
            ast::Block::Section {
                level,
                title,
                body,
                ..
            } => {
                let size_pt = if *level == 1 { 18.0 } else { 14.0 };
                let ctx = LayoutContext { fonts, size_pt };
                let items = build_paragraph_items(&ctx, title, FontRegistry::bold());
                let breaks = break_paragraph(&items, text_width, 10.0, 50.0);
                block_lines = items_to_lines(&items, &breaks, text_width);

                // Add pre-section padding
                *current_y += size_pt;

                // Process title lines
                add_lines_to_page(
                    block_lines,
                    layout,
                    current_y,
                    current_page_lines,
                    pages,
                    max_y,
                );
                block_lines = Vec::new(); // already handled

                // RECURSIVE: Process section body
                layout_blocks(body, fonts, layout, current_y, current_page_lines, pages);
            }
            ast::Block::VSpace { amount_pt, .. } => {
                *current_y += amount_pt;
            }
            ast::Block::PageBreak { .. } => {
                pages.push(LayoutPage {
                    width: layout.width_pt,
                    height: layout.height_pt,
                    lines: current_page_lines.clone(),
                    footnotes: Vec::new(),
                });
                current_page_lines.clear();
                *current_y = layout.margin_top;
            }
            ast::Block::MathBlock { node, .. } => {
                let math_layout = super::math::layout_math(
                    node,
                    fonts,
                    12.0,
                    super::math::LayoutStyle::Display,
                );
                // Center display math
                let x_offset = (text_width - math_layout.width) / 2.0;
                let mut boxes = math_layout.boxes;
                for b in &mut boxes {
                    b.x += x_offset;
                }
                block_lines.push(crate::layout::boxes::LayoutLine {
                    boxes,
                    width: math_layout.width,
                    height: math_layout.height,
                    depth: math_layout.depth,
                    baseline_y: 0.0,
                });
            }
            _ => {}
        }

        // Add remaining lines to page (e.g. from Paragraph or Section title if not cleared)
        add_lines_to_page(
            block_lines,
            layout,
            current_y,
            current_page_lines,
            pages,
            max_y,
        );

        // Post-block gap
        *current_y += 10.0;
    }
}

fn add_lines_to_page(
    lines: Vec<LayoutLine>,
    layout: &PageLayout,
    current_y: &mut f64,
    current_page_lines: &mut Vec<LayoutLine>,
    pages: &mut Vec<LayoutPage>,
    max_y: f64,
) {
    for mut line in lines {
        // Check page overflow
        if *current_y + line.height + line.depth > max_y && !current_page_lines.is_empty() {
            pages.push(LayoutPage {
                width: layout.width_pt,
                height: layout.height_pt,
                lines: current_page_lines.clone(),
                footnotes: Vec::new(),
            });
            current_page_lines.clear();
            *current_y = layout.margin_top;
        }

        // Reposition line absolutely relative to the page definition.
        *current_y += line.height;

        for box_ in &mut line.boxes {
            box_.x += layout.margin_left;
            box_.y += *current_y;
        }

        line.baseline_y = *current_y;
        current_page_lines.push(line.clone());

        *current_y += line.depth + 3.0; // Minimal line gap
    }
}
