//! Compilation cache for incremental recompilation.

use crate::ast;
use crate::fonts::loader::FontRegistry;
use crate::layout::boxes::LayoutLine;
use crate::layout::page::{LayoutPage, PageLayout};
use rustc_hash::FxHashMap;
use std::hash::{Hash, Hasher};

/// Cached compilation state for incremental recompilation.
#[allow(dead_code)]
#[derive(Debug, Default)]
pub struct CompilationCache {
    /// Hash of each top-level Block's source span bytes → laid-out lines.
    block_cache: FxHashMap<u64, Vec<LayoutLine>>,
    /// Resolved `\label` → target.
    labels: FxHashMap<String, LabelTarget>,
    /// Page break assignments: index into doc.body → page index.
    page_breaks: Vec<usize>,
    /// Footnote content per page.
    footnotes: Vec<Vec<Vec<LayoutLine>>>,
}

/// The target of a `\label` cross-reference.
#[derive(Debug, Clone)]
pub struct LabelTarget {
    pub page: usize,
    pub section_number: String,
}

/// Hash a block's source text for cache lookup.
pub fn hash_block_source(src: &str, block: &ast::Block) -> u64 {
    let span = block_span(block);
    let slice = &src[span.start..span.end.min(src.len())];
    let mut hasher = rustc_hash::FxHasher::default();
    slice.hash(&mut hasher);
    hasher.finish()
}

/// Incremental compilation using cached layout.
pub fn compile_with_cache(
    src: &str,
    doc: &ast::Document,
    fonts: &FontRegistry,
    page_layout: &PageLayout,
    cache: &mut CompilationCache,
) -> Vec<LayoutPage> {

    let max_y = page_layout.height_pt - page_layout.margin_bot;
    
    let mut new_block_cache = FxHashMap::default();
    let mut current_page_lines = Vec::new();
    let mut pages = Vec::new();
    let mut current_y = page_layout.margin_top;

    // Helper to add lines to pages with pagination logic
    let add_lines = |lines: Vec<LayoutLine>, current_y: &mut f64, current_page_lines: &mut Vec<LayoutLine>, pages: &mut Vec<LayoutPage>| {
        for mut line in lines {
            if *current_y + line.height + line.depth > max_y && !current_page_lines.is_empty() {
                pages.push(LayoutPage {
                    width: page_layout.width_pt,
                    height: page_layout.height_pt,
                    lines: current_page_lines.clone(),
                    footnotes: Vec::new(),
                });
                current_page_lines.clear();
                *current_y = page_layout.margin_top;
            }

            *current_y += line.height;
            for box_ in &mut line.boxes {
                // If it was cached, the X is already offset correctly but Y needs strict binding.
                // Reset Y offset safely
                box_.y = *current_y;
            }
            line.baseline_y = *current_y;
            current_page_lines.push(line.clone());
            *current_y += line.depth + 3.0;
        }
    };

    // Full doc generation using block substitution
    for block in &doc.body {
        let block_hash = hash_block_source(src, block);
        
        let block_lines = if let Some(cached_lines) = cache.block_cache.remove(&block_hash) {
            cached_lines
        } else {
            // Uncached block routing
            let temp_doc = ast::Document {
                span: doc.span,
                preamble: doc.preamble.clone(),
                body: vec![block.clone()],
            };
            
            // We use standard layout for this solitary block
            let single_page = crate::layout::page::layout_document(&temp_doc, fonts, page_layout);
            
            // Extract the purely laid-out lines 
            let mut extracted = Vec::new();
            for p in single_page {
                extracted.extend(p.lines);
            }
            
            extracted
        };

        match block {
            ast::Block::VSpace { amount_pt, .. } => {
                current_y += amount_pt;
            }
            ast::Block::PageBreak { .. } => {
                pages.push(LayoutPage {
                    width: page_layout.width_pt,
                    height: page_layout.height_pt,
                    lines: current_page_lines.clone(),
                    footnotes: Vec::new(),
                });
                current_page_lines.clear();
                current_y = page_layout.margin_top;
            }
            _ => {
                add_lines(block_lines.clone(), &mut current_y, &mut current_page_lines, &mut pages);
            }
        }
        
        current_y += 10.0; // Block gap
        new_block_cache.insert(block_hash, block_lines);
    }

    if !current_page_lines.is_empty() || pages.is_empty() {
        pages.push(LayoutPage {
            width: page_layout.width_pt,
            height: page_layout.height_pt,
            lines: current_page_lines,
            footnotes: Vec::new(),
        });
    }

    // Save newly active cache
    cache.block_cache = new_block_cache;

    pages
}

fn block_span(block: &ast::Block) -> crate::error::Span {
    match block {
        ast::Block::Paragraph { span, .. }
        | ast::Block::Section { span, .. }
        | ast::Block::List { span, .. }
        | ast::Block::Figure { span, .. }
        | ast::Block::Table { span, .. }
        | ast::Block::MathBlock { span, .. }
        | ast::Block::Verbatim { span, .. }
        | ast::Block::HRule { span }
        | ast::Block::PageBreak { span }
        | ast::Block::VSpace { span, .. }
        | ast::Block::RawCommand { span, .. } => *span,
    }
}
