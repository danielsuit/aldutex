//! # Aldutex — Pure-Rust LaTeX Typesetting Engine
//!
//! Named after Aldus Manutius (1449–1515), the Venetian printer who invented
//! italic type, the portable book, and the Aldine Press — the model of making
//! beautiful typesetting accessible to everyone.
//!
//! Aldutex is a zero-dependency-on-C, fully static LaTeX compiler that
//! produces PDF output from `.tex` source files. It features:
//!
//! - Full Knuth-Plass line breaking algorithm
//! - OpenType math layout from the MATH table
//! - Incremental compilation with block-level caching
//! - Rich `miette`-powered error diagnostics with source spans
//!
//! ## Quick Start
//!
//! ```no_run
//! let source = r#"\documentclass{article}
//! \begin{document}
//! Hello, world!
//! \end{document}"#;
//!
//! let (pdf, diagnostics) = aldutex::compile(source);
//! if diagnostics.has_errors() {
//!     for err in &diagnostics.errors {
//!         eprintln!("{err}");
//!     }
//! }
//! if let Some(bytes) = pdf {
//!     std::fs::write("output.pdf", bytes).unwrap();
//! }
//! ```

pub mod ast;
pub mod cache;
pub mod error;
pub mod fonts;
pub mod layout;
pub mod lexer;
pub mod parser;
pub mod renderer;

pub use ast::Document;
pub use cache::CompilationCache;
pub use error::{AldutexError, Diagnostics, Span};

/// Full compilation: source → PDF bytes.
///
/// Returns `(pdf_bytes, diagnostics)`. If `diagnostics.has_errors()`,
/// `pdf_bytes` may be `None` or contain a partial render.
pub fn compile(source: &str) -> (Option<Vec<u8>>, Diagnostics) {
    let tokens = lexer::Lexer::new(source).tokenize();
    let (doc, mut diagnostics) = parser::Parser::new(tokens, source).parse();

    if diagnostics.has_errors() {
        return (None, diagnostics);
    }

    let fonts = match fonts::loader::FontRegistry::new() {
        Ok(f) => f,
        Err(e) => {
            diagnostics.push_error(AldutexError::FontLoadFailed {
                reason: e.to_string(),
            });
            return (None, diagnostics);
        }
    };

    let page_layout = layout::page::PageLayout::from_document_class(&doc.preamble.document_class);
    let pages = layout::page::layout_document(&doc, &fonts, &page_layout);

    match renderer::pdf::render_to_pdf(&pages, &fonts, &page_layout) {
        Ok(bytes) => (Some(bytes), diagnostics),
        Err(e) => {
            diagnostics.push_error(AldutexError::RenderFailed {
                reason: e.to_string(),
            });
            (None, diagnostics)
        }
    }
}

/// Incremental compilation using a cache from a previous compile.
pub fn compile_incremental(
    source: &str,
    cache: &mut CompilationCache,
) -> (Option<Vec<u8>>, Diagnostics) {
    let tokens = lexer::Lexer::new(source).tokenize();
    let (doc, mut diagnostics) = parser::Parser::new(tokens, source).parse();

    if diagnostics.has_errors() {
        return (None, diagnostics);
    }

    let fonts = match fonts::loader::FontRegistry::new() {
        Ok(f) => f,
        Err(e) => {
            diagnostics.push_error(AldutexError::FontLoadFailed {
                reason: e.to_string(),
            });
            return (None, diagnostics);
        }
    };

    let page_layout = layout::page::PageLayout::from_document_class(&doc.preamble.document_class);
    let pages = cache::compile_with_cache(source, &doc, &fonts, &page_layout, cache);

    match renderer::pdf::render_to_pdf(&pages, &fonts, &page_layout) {
        Ok(bytes) => (Some(bytes), diagnostics),
        Err(e) => {
            diagnostics.push_error(AldutexError::RenderFailed {
                reason: e.to_string(),
            });
            (None, diagnostics)
        }
    }
}

/// Parse only — returns AST and diagnostics. No layout or PDF.
///
/// Used by the FluXTeX editor for real-time syntax error display.
pub fn parse(source: &str) -> (Document, Diagnostics) {
    let tokens = lexer::Lexer::new(source).tokenize();
    parser::Parser::new(tokens, source).parse()
}

/// Returns a new empty compilation cache.
pub fn new_cache() -> CompilationCache {
    CompilationCache::default()
}
