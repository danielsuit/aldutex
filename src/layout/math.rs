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
        ast::MathNode::Atom { char, class, .. } => {
            // In text/display math, ordinary alphabetic variables should be italicized.
            let font_id = if *class == ast::MathClass::Ordinary && char.is_ascii_alphabetic() {
                FontId::italic()
            } else {
                FontId::math()
            };
            let font = fonts.get(font_id);
            if let Some(g) = crate::fonts::shaper::shape_char(font, *char, size_pt) {
                let ascender = crate::fonts::metrics::ascender_pt(font, size_pt);
                let descender = crate::fonts::metrics::descender_pt(font, size_pt);
                
                boxes.push(LayoutBox {
                    x: g.x_offset,
                    y: g.y_offset,
                    content: BoxContent::Glyph {
                        font_id,
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
                let (left_space, right_space) = inter_atom_spacing(child, size_pt);
                current_x += left_space;

                let child_layout = layout_math(child, fonts, size_pt, _style);
                for mut b in child_layout.boxes {
                    b.x += current_x;
                    boxes.push(b);
                }
                current_x += child_layout.width;
                current_x += right_space;
                max_h = f64::max(max_h, child_layout.height);
                max_d = f64::max(max_d, child_layout.depth);
            }
        }
        ast::MathNode::Super { base, exp, .. } => {
            let base_layout = layout_math(base, fonts, size_pt, _style);
            let exp_layout = layout_math(exp, fonts, size_pt * 0.7, LayoutStyle::Script);
            let shift_up = size_pt * 0.55;
            let gap = size_pt * 0.08;

            boxes.extend(base_layout.boxes);
            for mut b in exp_layout.boxes {
                b.x += base_layout.width + gap;
                // Positive y is upward in this renderer's glyph coordinate space.
                b.y += shift_up;
                boxes.push(b);
            }

            return MathLayout {
                boxes,
                width: base_layout.width + gap + exp_layout.width,
                height: f64::max(base_layout.height, shift_up + exp_layout.height),
                depth: base_layout.depth,
            };
        }
        ast::MathNode::Sub { base, sub, .. } => {
            let base_layout = layout_math(base, fonts, size_pt, _style);
            let sub_layout = layout_math(sub, fonts, size_pt * 0.7, LayoutStyle::Script);
            let shift_down = size_pt * 0.25;
            let gap = size_pt * 0.08;

            boxes.extend(base_layout.boxes);
            for mut b in sub_layout.boxes {
                b.x += base_layout.width + gap;
                b.y -= shift_down;
                boxes.push(b);
            }

            return MathLayout {
                boxes,
                width: base_layout.width + gap + sub_layout.width,
                height: base_layout.height,
                depth: f64::max(base_layout.depth, shift_down + sub_layout.depth),
            };
        }
        ast::MathNode::SubSuper { base, sub, sup, .. } => {
            let base_layout = layout_math(base, fonts, size_pt, _style);
            let sub_layout = layout_math(sub, fonts, size_pt * 0.7, LayoutStyle::Script);
            let sup_layout = layout_math(sup, fonts, size_pt * 0.7, LayoutStyle::Script);
            let shift_up = size_pt * 0.55;
            let shift_down = size_pt * 0.25;
            let gap = size_pt * 0.08;

            boxes.extend(base_layout.boxes);
            for mut b in sup_layout.boxes {
                b.x += base_layout.width + gap;
                b.y += shift_up;
                boxes.push(b);
            }
            for mut b in sub_layout.boxes {
                b.x += base_layout.width + gap;
                b.y -= shift_down;
                boxes.push(b);
            }

            return MathLayout {
                boxes,
                width: base_layout.width + gap + f64::max(sub_layout.width, sup_layout.width),
                height: f64::max(base_layout.height, shift_up + sup_layout.height),
                depth: f64::max(base_layout.depth, shift_down + sub_layout.depth),
            };
        }
        ast::MathNode::Frac { num, den, .. } => {
            let script_size = size_pt * 0.85;
            let num_layout = layout_math(num, fonts, script_size, LayoutStyle::Script);
            let den_layout = layout_math(den, fonts, script_size, LayoutStyle::Script);

            let side_pad = size_pt * 0.15;
            let inner_width = f64::max(num_layout.width, den_layout.width);
            let total_width = inner_width + side_pad * 2.0;

            let num_shift_up = size_pt * 0.75;
            let den_shift_down = size_pt * 0.70;
            let bar_y = size_pt * 0.05;

            // Numerator centered over the bar.
            let num_x = side_pad + (inner_width - num_layout.width) / 2.0;
            for mut b in num_layout.boxes {
                b.x += num_x;
                b.y += num_shift_up;
                boxes.push(b);
            }

            // Draw a real fraction bar so it does not depend on font glyph shape.
            boxes.push(LayoutBox {
                x: side_pad,
                y: bar_y,
                content: BoxContent::Rule {
                    width: inner_width,
                    height: size_pt * 0.08,
                    depth: 0.0,
                },
            });

            // Denominator centered below the bar.
            let den_x = side_pad + (inner_width - den_layout.width) / 2.0;
            for mut b in den_layout.boxes {
                b.x += den_x;
                b.y -= den_shift_down;
                boxes.push(b);
            }

            return MathLayout {
                boxes,
                width: total_width,
                height: num_shift_up + num_layout.height,
                depth: den_shift_down + den_layout.depth,
            };
        }
        ast::MathNode::Sqrt { body, .. } => {
            let body_layout = layout_math(body, fonts, size_pt, _style);
            let math_font = fonts.get(FontId::math());
            let constants = crate::fonts::math_font::load_math_constants(math_font, size_pt)
                .unwrap_or_else(|_| crate::fonts::math_font::MathConstants::defaults(size_pt));
            let t = constants.radical_rule_thickness.max(size_pt * 0.05);
            let gap = constants.radical_vertical_gap.max(size_pt * 0.10);
            let extra_ascender = constants.radical_extra_ascender.max(size_pt * 0.04);
            let h = body_layout.height + gap + t + extra_ascender;

            // The radical sign is rendered as a single filled polygon: a slanted
            // diagonal stroke joined to a horizontal vinculum. This guarantees the
            // two parts form one connected shape regardless of font glyph design.
            let lean = h * 0.55;
            let sw = (t * 1.6).max(size_pt * 0.09);
            let body_pad_left = size_pt * 0.10;
            let trailing_pad = size_pt * 0.06;
            let hook_depth = size_pt * 0.12;
            let hook_width = sw * 1.4;
            let radicand_x = lean + sw + body_pad_left;
            let total_width = radicand_x + body_layout.width + trailing_pad;
            // Where the vinculum's bottom edge meets the diagonal's right (inside) edge.
            let inner_corner_x = sw + lean - lean * t / h;

            // Trace the outline clockwise, starting at the diagonal's outer-bottom
            // and ending at the descender hook's top-left back at the baseline.
            let polygon = vec![
                (0.0, 0.0),
                (lean, h),
                (total_width, h),
                (total_width, h - t),
                (inner_corner_x, h - t),
                (sw, 0.0),
                (sw, -hook_depth),
                (sw - hook_width, -hook_depth),
                (-hook_width, 0.0),
            ];

            boxes.push(LayoutBox {
                x: 0.0,
                y: 0.0,
                content: BoxContent::Path {
                    width: total_width,
                    height: h,
                    depth: hook_depth,
                    points: polygon,
                },
            });

            for mut b in body_layout.boxes {
                b.x += radicand_x;
                boxes.push(b);
            }

            return MathLayout {
                boxes,
                width: total_width,
                height: h,
                depth: f64::max(body_layout.depth, hook_depth),
            };
        }
        ast::MathNode::Operator { name, .. } => {
            return layout_text_run(name, FontId::math(), fonts, size_pt);
        }
        ast::MathNode::LargeOp { name, .. } => {
            let symbol = large_op_symbol(name);
            let text = symbol.to_string();
            return layout_text_run(&text, FontId::math(), fonts, size_pt);
        }
        ast::MathNode::Delimiter { kind, .. } => {
            let ch = delimiter_char(*kind);
            let text = ch.to_string();
            return layout_text_run(&text, FontId::math(), fonts, size_pt);
        }
        ast::MathNode::Text { content, .. } => {
            let text = inlines_plain_text(content);
            return layout_text_run(&text, FontId::regular(), fonts, size_pt);
        }
        ast::MathNode::Style { body, .. } => {
            return layout_math(body, fonts, size_pt, _style);
        }
        ast::MathNode::Over { body, .. } | ast::MathNode::Under { body, .. } => {
            return layout_math(body, fonts, size_pt, _style);
        }
        ast::MathNode::Row { children, .. } => {
            return layout_math(
                &ast::MathNode::Group {
                    children: children.clone(),
                    span: crate::error::Span::new(0, 0),
                },
                fonts,
                size_pt,
                _style,
            );
        }
        ast::MathNode::Matrix { rows, .. } => {
            // Fallback matrix layout: flatten rows inline, preserving separators.
            let mut flat_children = Vec::new();
            for (ri, row) in rows.iter().enumerate() {
                if ri > 0 {
                    flat_children.push(ast::MathNode::Atom {
                        char: ';',
                        class: ast::MathClass::Punct,
                        span: crate::error::Span::new(0, 0),
                    });
                }
                for (ci, cell) in row.iter().enumerate() {
                    if ci > 0 {
                        flat_children.push(ast::MathNode::Atom {
                            char: ',',
                            class: ast::MathClass::Punct,
                            span: crate::error::Span::new(0, 0),
                        });
                    }
                    flat_children.push(cell.clone());
                }
            }
            return layout_math(
                &ast::MathNode::Group {
                    children: flat_children,
                    span: crate::error::Span::new(0, 0),
                },
                fonts,
                size_pt,
                _style,
            );
        }
    }

    MathLayout {
        boxes,
        width: current_x,
        height: max_h,
        depth: max_d,
    }
}

fn inter_atom_spacing(node: &ast::MathNode, size_pt: f64) -> (f64, f64) {
    if let ast::MathNode::Atom { class, .. } = node {
        match class {
            ast::MathClass::Binary => {
                let s = size_pt * 0.18;
                (s, s)
            }
            ast::MathClass::Relation => {
                let s = size_pt * 0.24;
                (s, s)
            }
            ast::MathClass::Punct => (0.0, size_pt * 0.12),
            _ => (0.0, 0.0),
        }
    } else {
        (0.0, 0.0)
    }
}

fn layout_text_run(
    text: &str,
    font_id: FontId,
    fonts: &FontRegistry,
    size_pt: f64,
) -> MathLayout {
    let mut boxes = Vec::new();
    let mut current_x = 0.0;
    let font = fonts.get(font_id);
    let ascender = crate::fonts::metrics::ascender_pt(font, size_pt);
    let descender = crate::fonts::metrics::descender_pt(font, size_pt);
    let glyphs = crate::fonts::shaper::shape_text(font, text, size_pt, rustybuzz::Direction::LeftToRight);

    for g in glyphs {
        boxes.push(LayoutBox {
            x: current_x + g.x_offset,
            y: g.y_offset,
            content: BoxContent::Glyph {
                font_id,
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

    MathLayout {
        boxes,
        width: current_x,
        height: ascender,
        depth: descender,
    }
}

fn delimiter_char(kind: ast::DelimKind) -> char {
    match kind {
        ast::DelimKind::LParen => '(',
        ast::DelimKind::RParen => ')',
        ast::DelimKind::LBracket => '[',
        ast::DelimKind::RBracket => ']',
        ast::DelimKind::LBrace => '{',
        ast::DelimKind::RBrace => '}',
        ast::DelimKind::LFloor => '⌊',
        ast::DelimKind::RFloor => '⌋',
        ast::DelimKind::LCeil => '⌈',
        ast::DelimKind::RCeil => '⌉',
        ast::DelimKind::LAngle => '⟨',
        ast::DelimKind::RAngle => '⟩',
        ast::DelimKind::Vert => '|',
        ast::DelimKind::DoubleVert => '‖',
        ast::DelimKind::Dot => '.',
    }
}

fn large_op_symbol(name: &str) -> char {
    match name {
        "sum" => '∑',
        "prod" => '∏',
        "coprod" => '∐',
        "bigcup" => '⋃',
        "bigcap" => '⋂',
        "bigoplus" => '⊕',
        "bigotimes" => '⊗',
        "int" => '∫',
        "oint" => '∮',
        "iint" => '∬',
        "iiint" => '∭',
        _ => '∗',
    }
}

fn inlines_plain_text(inlines: &[ast::Inline]) -> String {
    let mut out = String::new();
    for inline in inlines {
        match inline {
            ast::Inline::Text { content, .. } => out.push_str(content),
            ast::Inline::NonBreakingSpace { .. } => out.push(' '),
            _ => {}
        }
    }
    out
}
