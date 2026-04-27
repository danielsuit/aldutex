//! Layout box model: positioned glyphs, rules, images, and links.

use crate::fonts::loader::FontId;
use std::sync::Arc;

/// A positioned layout element on a page.
#[derive(Debug, Clone)]
pub struct LayoutBox {
    /// X position from left edge of page content area.
    pub x: f64,
    /// Y position from top edge of page content area (baseline).
    pub y: f64,
    /// The content of this box.
    pub content: BoxContent,
}

/// The content type of a layout box.
#[derive(Debug, Clone)]
pub enum BoxContent {
    /// A single glyph from a font.
    Glyph {
        font_id: FontId,
        glyph_id: u16,
        size_pt: f64,
        /// Horizontal advance.
        width: f64,
        /// Vertical offset (relative to baseline).
        x_offset: f64,
        /// Vertical offset (relative to baseline).
        y_offset: f64,
        height: f64,
        depth: f64,
    },
    /// A filled rectangle (for rules, fraction bars, etc.).
    Rule { width: f64, height: f64, depth: f64 },
    /// An embedded image.
    Image {
        data: Arc<Vec<u8>>,
        format: ImageFormat,
        width: f64,
        height: f64,
    },
    /// A hyperlink wrapping child boxes.
    Link {
        url: String,
        children: Vec<LayoutBox>,
        width: f64,
        height: f64,
    },
}

/// Supported image formats.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ImageFormat {
    Png,
    Jpeg,
}

/// A laid-out line of text.
#[derive(Debug, Clone)]
pub struct LayoutLine {
    /// The boxes in this line.
    pub boxes: Vec<LayoutBox>,
    /// Actual width of the line after justification.
    pub width: f64,
    /// Maximum height above baseline.
    pub height: f64,
    /// Maximum depth below baseline (positive value).
    pub depth: f64,
    /// Y position of baseline on the page.
    pub baseline_y: f64,
}
