//! Error types and diagnostics for the Aldutex typesetting engine.
//!
//! Every module in Aldutex propagates errors through `miette::Result<T>` or
//! collects them into the [`Diagnostics`] struct. No `unwrap()` or `expect()`
//! is permitted outside `#[cfg(test)]` blocks.

use miette::{Diagnostic, SourceSpan};
use serde::{Deserialize, Serialize};
use thiserror::Error;

/// A byte-offset span into the original source string.
///
/// All AST nodes carry a `Span` so that error messages can point to the
/// exact location in the user's `.tex` source. Spans are also used for
/// incremental hashing — no span means no cache key.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Span {
    /// Byte offset of the first character (inclusive).
    pub start: usize,
    /// Byte offset one past the last character (exclusive).
    pub end: usize,
}

impl Span {
    /// Create a new span from byte offsets `[start, end)`.
    pub fn new(start: usize, end: usize) -> Self {
        Self { start, end }
    }

    /// Convert to a `miette::SourceSpan` for diagnostic display.
    pub fn to_source_span(self) -> SourceSpan {
        (self.start, self.end - self.start).into()
    }

    /// Merge two spans into one that covers both.
    pub fn merge(self, other: Span) -> Span {
        Span {
            start: self.start.min(other.start),
            end: self.end.max(other.end),
        }
    }
}

/// All error types produced during lexing, parsing, layout, and rendering.
#[derive(Debug, Clone, Error, Diagnostic)]
pub enum AldutexError {
    #[error("Unexpected token '{token}' at this position")]
    #[diagnostic(
        code(aldutex::parse::unexpected_token),
        help("Expected one of: {expected}")
    )]
    UnexpectedToken {
        token: String,
        expected: String,
        #[source_code]
        src: String,
        #[label("unexpected token here")]
        span: SourceSpan,
    },

    #[error("Unmatched \\begin{{{env}}}")]
    #[diagnostic(
        code(aldutex::parse::unmatched_begin),
        help("Add \\end{{{env}}} to close this environment")
    )]
    UnmatchedBegin {
        env: String,
        #[source_code]
        src: String,
        #[label("\\begin{{{env}}} opened here")]
        span: SourceSpan,
    },

    #[error("Unmatched \\end{{{env}}}")]
    #[diagnostic(code(aldutex::parse::unmatched_end))]
    UnmatchedEnd {
        env: String,
        #[source_code]
        src: String,
        #[label("\\end{{{env}}} has no matching \\begin")]
        span: SourceSpan,
    },

    #[error("Unknown command '\\{name}'")]
    #[diagnostic(
        code(aldutex::parse::unknown_command),
        severity(Warning),
        help("This command will be ignored in the output")
    )]
    UnknownCommand {
        name: String,
        #[source_code]
        src: String,
        #[label("unknown command")]
        span: SourceSpan,
    },

    #[error("Font loading failed: {reason}")]
    #[diagnostic(code(aldutex::font::load_failed))]
    FontLoadFailed { reason: String },

    #[error("Math environment error: {reason}")]
    #[diagnostic(code(aldutex::math::error))]
    MathError {
        reason: String,
        #[source_code]
        src: String,
        #[label("in this math expression")]
        span: SourceSpan,
    },

    #[error("PDF rendering failed: {reason}")]
    #[diagnostic(code(aldutex::render::failed))]
    RenderFailed { reason: String },
}

/// A collection of errors and warnings from a single compilation pass.
///
/// Non-fatal issues are collected as warnings; only errors prevent PDF output.
/// The compiler never panics — all problems end up here.
#[derive(Debug, Default)]
pub struct Diagnostics {
    /// Fatal errors that prevent correct output.
    pub errors: Vec<AldutexError>,
    /// Non-fatal warnings (e.g., unknown commands that are skipped).
    pub warnings: Vec<AldutexError>,
}

impl Diagnostics {
    /// Returns `true` if any fatal errors were recorded.
    pub fn has_errors(&self) -> bool {
        !self.errors.is_empty()
    }

    /// Record a fatal error.
    pub fn push_error(&mut self, e: AldutexError) {
        self.errors.push(e);
    }

    /// Record a non-fatal warning.
    pub fn push_warning(&mut self, w: AldutexError) {
        self.warnings.push(w);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn span_new_and_accessors() {
        let s = Span::new(10, 20);
        assert_eq!(s.start, 10);
        assert_eq!(s.end, 20);
    }

    #[test]
    fn span_merge() {
        let a = Span::new(5, 15);
        let b = Span::new(10, 25);
        let merged = a.merge(b);
        assert_eq!(merged.start, 5);
        assert_eq!(merged.end, 25);
    }

    #[test]
    fn span_to_source_span() {
        let s = Span::new(3, 8);
        let ss = s.to_source_span();
        // SourceSpan stores (offset, length)
        assert_eq!(ss.offset(), 3);
        assert_eq!(ss.len(), 5);
    }

    #[test]
    fn diagnostics_empty_by_default() {
        let d = Diagnostics::default();
        assert!(!d.has_errors());
        assert!(d.errors.is_empty());
        assert!(d.warnings.is_empty());
    }

    #[test]
    fn diagnostics_push_error() {
        let mut d = Diagnostics::default();
        d.push_error(AldutexError::FontLoadFailed {
            reason: "test".to_string(),
        });
        assert!(d.has_errors());
        assert_eq!(d.errors.len(), 1);
    }

    #[test]
    fn diagnostics_push_warning() {
        let mut d = Diagnostics::default();
        d.push_warning(AldutexError::UnknownCommand {
            name: "foo".to_string(),
            src: "\\foo".to_string(),
            span: Span::new(0, 4).to_source_span(),
        });
        assert!(!d.has_errors());
        assert_eq!(d.warnings.len(), 1);
    }
}
