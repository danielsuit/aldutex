//! Knuth-Plass line breaking algorithm.
//!
//! Implements the optimal paragraph breaking algorithm from
//! "Breaking Paragraphs into Lines" (Knuth & Plass, 1981).

use crate::layout::boxes::{LayoutBox, LayoutLine};

/// An item in the Knuth-Plass item list.
#[derive(Debug, Clone)]
pub enum Item {
    /// A fixed-width box (word, glyph run).
    Box {
        width: f64,
        content: Vec<LayoutBox>, // Using the real layout boxes
                                 // Normally, a box's height/depth are used for the resulting line
    },
    /// Flexible inter-word glue.
    Glue {
        width: f64,
        stretch: f64,
        shrink: f64,
    },
    /// A penalty for breaking at this position.
    Penalty {
        width: f64,
        penalty: f64,
        flagged: bool,
    },
}

impl Item {
    /// Helper to get the natural width
    pub fn width(&self) -> f64 {
        match self {
            Item::Box { width, .. } => *width,
            Item::Glue { width, .. } => *width,
            Item::Penalty { width, .. } => *width,
        }
    }

    /// Are we a forced break?
    pub fn is_forced_break(&self) -> bool {
        match self {
            Item::Penalty { penalty, .. } => *penalty <= -10000.0,
            _ => false,
        }
    }
}

/// A node in the active list of the dynamic programming algorithm.
#[derive(Debug, Clone)]
struct ActiveNode {
    /// The index in the items array where this line starts.
    position: usize,
    /// The fitness class of this line (0: tight, 1: normal, 2: loose, 3: very loose).
    fitness_class: usize,
    /// Total demerits up to this point.
    total_demerits: f64,
    /// The line number from the start of the paragraph.
    line_number: usize,
    /// The index of the previous active node that led to this node optimally.
    previous_node: Option<Box<ActiveNode>>,
}

/// Represents the sum of widths, stretch, and shrink at a given point in the list.
#[derive(Default, Clone, Copy)]
struct Sums {
    width: f64,
    stretch: f64,
    shrink: f64,
}

const INFINITE_PENALTY: f64 = 10000.0;
const NEG_INFINITE_PENALTY: f64 = -10000.0;
const INFINITE_DEMERITS: f64 = 1e20;

/// Break a paragraph into optimally justified lines using the Knuth-Plass algorithm.
/// Returns a list of item indices where line breaks occur.
pub fn break_paragraph(
    items: &[Item],
    line_width: f64,
    line_penalty: f64,
    _hyphen_penalty: f64,
) -> Vec<usize> {
    if items.is_empty() {
        return Vec::new();
    }

    // Pre-calculate running sums for fast queries
    let mut running_sums = vec![Sums::default(); items.len() + 1];
    let mut current_sum = Sums::default();
    for (i, item) in items.iter().enumerate() {
        match item {
            Item::Box { width, .. } => current_sum.width += width,
            Item::Glue {
                width,
                stretch,
                shrink,
            } => {
                current_sum.width += width;
                current_sum.stretch += stretch;
                current_sum.shrink += shrink;
            }
            Item::Penalty { .. } => {
                // For sums, we usually don't add penalty widths permanently unless a break occurs,
                // but Knuth-Plass sums exclude penalty widths. Penalty width is added at break time.
            }
        }
        running_sums[i + 1] = current_sum;
    }

    // Initialize active nodes list
    let mut active_nodes = vec![ActiveNode {
        position: 0,
        fitness_class: 1, // normal
        total_demerits: 0.0,
        line_number: 0,
        previous_node: None,
    }];

    for (i, item) in items.iter().enumerate() {
        // Is this a legal breakpoint?
        let is_legal_break = match item {
            Item::Penalty { penalty, .. } => *penalty < INFINITE_PENALTY,
            Item::Glue { .. } => {
                if i > 0 {
                    matches!(items[i - 1], Item::Box { .. })
                } else {
                    false
                }
            }
            _ => false,
        };

        if !is_legal_break {
            continue;
        }

        let is_forced = item.is_forced_break();

        // Used to track the best node per fitness class to add after evaluating current item
        let mut best_for_class: [Option<(f64, ActiveNode)>; 4] = [None, None, None, None];

        // Evaluate all active nodes
        // Retain nodes unless they are too far back (ratio < -1)
        active_nodes.retain(|active| {
            // Distance from active.position to i
            let available_width = line_width;

            let sum_width = running_sums[i].width - running_sums[active.position].width;
            let sum_stretch = running_sums[i].stretch - running_sums[active.position].stretch;
            let sum_shrink = running_sums[i].shrink - running_sums[active.position].shrink;

            let mut line_width_used = sum_width;

            // Add penalty width if breaking here and it's a penalty
            if let Item::Penalty { width, .. } = item {
                line_width_used += width;
            }

            // Adjustment ratio
            let mut ratio = 0.0;
            let width_diff = available_width - line_width_used;

            if width_diff < 0.0 {
                // Shrinking
                if sum_shrink > 0.0 {
                    ratio = width_diff / sum_shrink;
                } else {
                    ratio = -INFINITE_DEMERITS; // Can't shrink enough
                }
            } else if width_diff > 0.0 {
                // Stretching
                if sum_stretch > 0.0 {
                    ratio = width_diff / sum_stretch;
                } else {
                    // Infinite stretch for incomplete lines
                    if is_forced {
                        ratio = 0.0;
                    } else {
                        ratio = INFINITE_DEMERITS; // Too short, no stretch
                    }
                }
            }

            // Forced breaks force the break regardless of ratio
            if ratio < -1.0 && !is_forced {
                // This active node is too far away to form a valid line
                return false;
            }

            // Calculate badness and demerits if valid
            if ratio >= -1.0 || is_forced {
                let badness = if is_forced && width_diff > 0.0 {
                    0.0
                } else {
                    let r = ratio.abs();
                    if r > 10.0 {
                        INFINITE_DEMERITS // extremely bad
                    } else {
                        100.0 * r.powi(3)
                    }
                };

                let penalty_val = match item {
                    Item::Penalty { penalty, .. } => *penalty,
                    _ => 0.0,
                };

                // Compute demerits
                let mut demerits = if penalty_val >= 0.0 {
                    (line_penalty + badness).powi(2) + penalty_val.powi(2)
                } else if penalty_val > NEG_INFINITE_PENALTY {
                    (line_penalty + badness).powi(2) - penalty_val.powi(2)
                } else {
                    (line_penalty + badness).powi(2)
                };

                // Add fitness penalty (if fitness classes differ by more than 1)
                let fitness_class = if ratio < -0.5 {
                    0 // tight
                } else if ratio <= 0.5 {
                    1 // normal
                } else if ratio <= 1.0 {
                    2 // loose
                } else {
                    3 // very loose
                };

                if (fitness_class as i32 - active.fitness_class as i32).abs() > 1 {
                    // Add large penalty for adjacent lines with very different fitness
                    demerits += 3000.0;
                }

                let total_demerits = active.total_demerits + demerits;
                // Record best per fitness class
                let curr_best = &mut best_for_class[fitness_class];
                match curr_best {
                    None => {
                        *curr_best = Some((total_demerits, active.clone()));
                    }
                    Some((best_demerits, _)) if total_demerits < *best_demerits => {
                        *curr_best = Some((total_demerits, active.clone()));
                    }
                    _ => {}
                }
            }

            true // keep this node for future breaks
        });

        // Add the best nodes to active list
        for (class, best) in best_for_class.iter().enumerate() {
            if let Some((demerits, previous)) = best {
                active_nodes.push(ActiveNode {
                    position: i,
                    fitness_class: class,
                    total_demerits: *demerits,
                    line_number: previous.line_number + 1,
                    previous_node: Some(Box::new(previous.clone())),
                });
            }
        }
    }

    // Find the end node with minimum demerits. Usually the last item is a forced break.
    let last_pos = items.len().saturating_sub(1);
    let best_end = active_nodes
        .iter()
        .filter(|a| a.position == last_pos)
        .min_by(|a, b| a.total_demerits.partial_cmp(&b.total_demerits).unwrap());

    let mut breaks = Vec::new();
    if let Some(current) = best_end {
        let mut curr_clone = current.clone();
        while curr_clone.position > 0 {
            breaks.push(curr_clone.position);
            if let Some(prev) = curr_clone.previous_node {
                curr_clone = *prev;
            } else {
                break;
            }
        }
    }

    breaks.reverse();
    breaks
}

/// Convert break positions into laid-out lines.
pub fn items_to_lines(items: &[Item], breaks: &[usize], line_width: f64) -> Vec<LayoutLine> {
    let mut lines = Vec::new();
    let mut start_idx = 0;
    let mut current_y = 0.0;

    for &end_idx in breaks {
        // Collect items for this line
        let line_items = &items[start_idx..=end_idx];

        // Measure line width
        let mut width = 0.0;
        let mut stretch = 0.0;
        let mut shrink = 0.0;

        let mut end_box_idx = line_items.len();

        for (i, item) in line_items.iter().enumerate() {
            match item {
                Item::Box { width: w, .. } => {
                    width += w;
                    end_box_idx = i;
                }
                Item::Glue {
                    width: w,
                    stretch: st,
                    shrink: sh,
                } => {
                    if i < end_box_idx || i < line_items.len() - 1 {
                        // Normally trailing glue is discarded or we don't compute its width
                        // if at the break point, but for now we simply include internal glue.
                        width += w;
                        stretch += st;
                        shrink += sh;
                    }
                }
                Item::Penalty { width: w, .. } => {
                    if i == line_items.len() - 1 {
                        width += w; // Add penalty width if breaking here
                    }
                }
            }
        }

        // Justify
        let mut ratio = 0.0;
        if width < line_width && stretch > 0.0 {
            ratio = (line_width - width) / stretch;
        } else if width > line_width && shrink > 0.0 {
            ratio = (line_width - width) / shrink;
        }

        // Lay out boxes
        let mut current_x = 0.0;
        let mut max_ascender = 0.0;
        let mut max_descender = 0.0;
        let mut out_boxes = Vec::new();

        for item in line_items {
            match item {
                Item::Box { width: w, content } => {
                    // Update bounds based on content
                    for b in content {
                        // Find max ascender/descender (approximations for layout bounds)
                        // In reality, BoxContent has font metrics but we default here
                        let h = 10.0;
                        let d = 2.0;
                        if h > max_ascender {
                            max_ascender = h;
                        }
                        if d > max_descender {
                            max_descender = d;
                        }

                        let mut positioned = b.clone();
                        positioned.x += current_x;
                        positioned.y += current_y;
                        out_boxes.push(positioned);
                    }
                    current_x += w;
                }
                Item::Glue {
                    width: w,
                    stretch: st,
                    shrink: sh,
                } => {
                    if ratio >= 0.0 {
                        current_x += w + st * ratio;
                    } else {
                        current_x += w + sh * ratio;
                    }
                }
                Item::Penalty { .. } => {
                    // No horizontal advance internally
                }
            }
        }

        // Advance Y for next line based on bounds or leading
        let line_height = f64::max(max_ascender + max_descender, 12.0); // min leading

        lines.push(LayoutLine {
            boxes: out_boxes,
            width: current_x,
            height: max_ascender,
            depth: max_descender,
            baseline_y: current_y,
        });

        current_y += line_height * 1.2; // 1.2 line spacing
        start_idx = end_idx + 1;
    }

    lines
}
