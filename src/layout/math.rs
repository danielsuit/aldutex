//! Math expression layout.

use crate::ast;
use crate::fonts::loader::{FontId, FontRegistry};
use crate::layout::boxes::{BoxContent, LayoutBox};

/// Layout style for math expressions.
#[derive(Debug, Clone, Copy)]
pub enum LayoutStyle {
    Display,
    Text,
    Script,
    ScriptScript,
}

/// A laid-out math node.
#[derive(Debug, Clone)]
pub struct MathLayout {
    pub boxes: Vec<LayoutBox>,
    pub width: f64,
    pub height: f64,
    pub depth: f64,
}

/// Lay out a math node recursively.
pub fn layout_math(
    node: &ast::MathNode,
    fonts: &FontRegistry,
    size_pt: f64,
    _style: LayoutStyle,
) -> MathLayout {
    let mut boxes = Vec::new();
    let mut current_x = 0.0;
    let mut max_h = 0.0;
    let mut max_d = 0.0;

    match node {
        ast::MathNode::Atom { char, .. } => {
            let font = fonts.get(FontId::math());
            if let Some(g) = crate::fonts::shaper::shape_char(font, *char, size_pt) {
                let ascender = crate::fonts::metrics::ascender_pt(font, size_pt);
                let descender = crate::fonts::metrics::descender_pt(font, size_pt);
                
                boxes.push(LayoutBox {
                    x: g.x_offset,
                    y: g.y_offset,
                    content: BoxContent::Glyph {
                        font_id: FontId::math(),
                        glyph_id: g.glyph_id,
                        size_pt,
                        width: g.x_advance,
                        x_offset: g.x_offset,
                        y_offset: g.y_offset,
                        height: ascender,
                        depth: descender,
                    },
                });
                
                return MathLayout {
                    boxes,
                    width: g.x_advance,
                    height: ascender,
                    depth: descender,
                };
            }
        }
        ast::MathNode::Number { value, .. } | ast::MathNode::Ident { name: value, .. } => {
            let font = fonts.get(FontId::math());
            let glyphs = crate::fonts::shaper::shape_text(font, value, size_pt, rustybuzz::Direction::LeftToRight);
            let ascender = crate::fonts::metrics::ascender_pt(font, size_pt);
            let descender = crate::fonts::metrics::descender_pt(font, size_pt);
            
            for g in glyphs {
                boxes.push(LayoutBox {
                    x: current_x + g.x_offset,
                    y: g.y_offset,
                    content: BoxContent::Glyph {
                        font_id: FontId::math(),
                        glyph_id: g.glyph_id,
                        size_pt,
                        width: g.x_advance,
                        x_offset: g.x_offset,
                        y_offset: g.y_offset,
                        height: ascender,
                        depth: descender,
                    },
                });
                current_x += g.x_advance;
            }
            
            return MathLayout {
                boxes,
                width: current_x,
                height: ascender,
                depth: descender,
            };
        }
        ast::MathNode::Group { children, .. } => {
            for child in children {
                let child_layout = layout_math(child, fonts, size_pt, _style);
                for mut b in child_layout.boxes {
                    b.x += current_x;
                    boxes.push(b);
                }
                current_x += child_layout.width;
                max_h = f64::max(max_h, child_layout.height);
                max_d = f64::max(max_d, child_layout.depth);
            }
        }
        _ => {
            // Simplified: treat others as empty for now to avoid complexity in this step
        }
    }

    MathLayout {
        boxes,
        width: current_x,
        height: max_h,
        depth: max_d,
    }
}
