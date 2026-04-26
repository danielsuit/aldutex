//! AST definitions for the Aldutex typesetting engine.
//!
//! This module contains all AST node types used by the parser and layout engine.
//! No logic lives here — only pure data definitions. Every node carries a [`Span`]
//! for error reporting and incremental hashing.

use crate::error::Span;
use serde::{Deserialize, Serialize};

// ── Document root ──────────────────────────────────────────────

/// The root of a parsed LaTeX document.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Document {
    pub span: Span,
    pub preamble: Preamble,
    pub body: Vec<Block>,
}

/// Everything between `\documentclass` and `\begin{document}`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Preamble {
    pub document_class: DocumentClass,
    pub packages: Vec<Package>,
    pub metadata: DocumentMetadata,
}

/// The `\documentclass[options]{name}` declaration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DocumentClass {
    /// Class name: `"article"`, `"report"`, `"book"`, `"beamer"`.
    pub name: String,
    /// Options: `["12pt", "a4paper", "twoside"]`.
    pub options: Vec<String>,
    pub span: Span,
}

/// A `\usepackage[options]{name}` declaration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Package {
    pub name: String,
    pub options: Vec<String>,
    pub span: Span,
}

/// Metadata declared in the preamble: `\title`, `\author`, `\date`, `\abstract`.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct DocumentMetadata {
    pub title: Option<Vec<Inline>>,
    pub author: Option<Vec<Inline>>,
    pub date: Option<Vec<Inline>>,
    pub abstract_: Option<Vec<Block>>,
}

// ── Block-level nodes ──────────────────────────────────────────

/// A block-level element in the document body.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Block {
    /// A paragraph of inline content.
    Paragraph { inlines: Vec<Inline>, span: Span },

    /// A sectioning command (`\section`, `\subsection`, etc.).
    /// Level: 1=`\section`, 2=`\subsection`, 3=`\subsubsection`,
    ///        4=`\paragraph`, 5=`\subparagraph`.
    Section {
        level: u8,
        title: Vec<Inline>,
        label: Option<String>,
        body: Vec<Block>,
        span: Span,
    },

    /// An itemize, enumerate, or description list.
    List {
        kind: ListKind,
        items: Vec<ListItem>,
        span: Span,
    },

    /// A figure environment with optional caption and label.
    Figure {
        content: Vec<Block>,
        caption: Option<Vec<Inline>>,
        label: Option<String>,
        placement: String,
        span: Span,
    },

    /// A table/tabular environment.
    Table {
        /// Column spec, e.g. `"l c r | l"`.
        spec: String,
        rows: Vec<TableRow>,
        caption: Option<Vec<Inline>>,
        label: Option<String>,
        span: Span,
    },

    /// Display math: `\[...\]` or `equation` environment.
    MathBlock { node: MathNode, span: Span },

    /// Verbatim or lstlisting environment.
    Verbatim { content: String, span: Span },

    /// A horizontal rule (`\hrule` or `\hline`).
    HRule { span: Span },

    /// A page break (`\newpage`, `\clearpage`).
    PageBreak { span: Span },

    /// Vertical space (`\vspace{...}`).
    VSpace { amount_pt: f64, span: Span },

    /// An unrecognized command, preserved for warning display.
    RawCommand {
        name: String,
        args: Vec<Arg>,
        span: Span,
    },
}

/// The kind of list environment.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ListKind {
    Itemize,
    Enumerate,
    Description,
}

/// A single `\item` inside a list.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ListItem {
    /// Optional custom label: `\item[label]`.
    pub label: Option<Vec<Inline>>,
    /// The block content of this item.
    pub content: Vec<Block>,
    pub span: Span,
}

/// A row inside a tabular environment.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TableRow {
    pub cells: Vec<TableCell>,
    pub span: Span,
}

/// A single cell in a table row.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TableCell {
    pub content: Vec<Inline>,
    /// Column span for `\multicolumn{n}{spec}{content}`.
    pub colspan: u8,
    pub span: Span,
}

// ── Inline nodes ───────────────────────────────────────────────

/// An inline element within a paragraph or other inline context.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Inline {
    /// Plain text run.
    Text { content: String, span: Span },

    /// Bold text: `\textbf{...}`.
    Bold { content: Vec<Inline>, span: Span },

    /// Italic text: `\textit{...}`.
    Italic { content: Vec<Inline>, span: Span },

    /// Bold italic text: `\textbf{\textit{...}}` or vice versa.
    BoldItalic { content: Vec<Inline>, span: Span },

    /// Underlined text: `\underline{...}`.
    Underline { content: Vec<Inline>, span: Span },

    /// Monospace text: `\texttt{...}`.
    Monospace { content: Vec<Inline>, span: Span },

    /// Small caps: `\textsc{...}`.
    SmallCaps { content: Vec<Inline>, span: Span },

    /// Emphasis: `\emph{...}` (toggles italic/upright).
    Emph { content: Vec<Inline>, span: Span },

    /// Inline math: `$...$`.
    Math { node: MathNode, span: Span },

    /// A hyperlink: `\href{url}{text}`.
    Link {
        url: String,
        text: Vec<Inline>,
        span: Span,
    },

    /// A cross-reference: `\ref{label}`.
    Ref { label: String, span: Span },

    /// A citation: `\cite{key1,key2}`.
    Citation { keys: Vec<String>, span: Span },

    /// A footnote reference (superscript number).
    FootnoteRef { index: usize, span: Span },

    /// A non-breaking space: `~`.
    NonBreakingSpace { span: Span },

    /// Horizontal space: `\hspace{...}`.
    HSpace { amount_pt: f64, span: Span },

    /// A line break: `\\`.
    LineBreak { span: Span },

    /// An unrecognized inline command, preserved for warning display.
    RawInlineCmd {
        name: String,
        args: Vec<Arg>,
        span: Span,
    },
}

// ── Math nodes ─────────────────────────────────────────────────

/// A node in a math expression tree.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum MathNode {
    /// A single character with a math classification.
    Atom {
        char: char,
        class: MathClass,
        span: Span,
    },

    /// A run of digits (possibly with a decimal point).
    Number { value: String, span: Span },

    /// A multi-character identifier.
    Ident { name: String, span: Span },

    /// A named operator: `\sin`, `\cos`, `\lim`, etc.
    Operator { name: String, span: Span },

    /// A fraction: `\frac{num}{den}`.
    Frac {
        num: Box<MathNode>,
        den: Box<MathNode>,
        span: Span,
    },

    /// A square root: `\sqrt[degree]{body}`.
    Sqrt {
        degree: Option<Box<MathNode>>,
        body: Box<MathNode>,
        span: Span,
    },

    /// Superscript: `base^{exp}`.
    Super {
        base: Box<MathNode>,
        exp: Box<MathNode>,
        span: Span,
    },

    /// Subscript: `base_{sub}`.
    Sub {
        base: Box<MathNode>,
        sub: Box<MathNode>,
        span: Span,
    },

    /// Both subscript and superscript: `base_{sub}^{sup}`.
    SubSuper {
        base: Box<MathNode>,
        sub: Box<MathNode>,
        sup: Box<MathNode>,
        span: Span,
    },

    /// A brace-delimited group: `{children}`.
    Group { children: Vec<MathNode>, span: Span },

    /// A large operator: `\sum`, `\int`, `\prod`, etc.
    LargeOp {
        name: String,
        limits: bool,
        span: Span,
    },

    /// A delimiter: parenthesis, bracket, brace, etc.
    Delimiter { kind: DelimKind, span: Span },

    /// An accent over the body: `\hat`, `\vec`, `\bar`, etc.
    Over {
        body: Box<MathNode>,
        accent: String,
        span: Span,
    },

    /// An accent under the body: `\underbrace`, etc.
    Under {
        body: Box<MathNode>,
        accent: String,
        span: Span,
    },

    /// Text inside math: `\text{...}`.
    Text { content: Vec<Inline>, span: Span },

    /// A math style command: `\mathbf`, `\mathit`, `\mathbb`, etc.
    Style {
        style: MathStyle,
        body: Box<MathNode>,
        span: Span,
    },

    /// A row in an align environment.
    Row { children: Vec<MathNode>, span: Span },

    /// A matrix or cases environment.
    Matrix {
        rows: Vec<Vec<MathNode>>,
        /// The environment name: `"matrix"`, `"pmatrix"`, `"bmatrix"`, etc.
        env: String,
        span: Span,
    },
}

/// Classification of a math atom for inter-atom spacing.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum MathClass {
    Ordinary,
    Binary,
    Relation,
    Open,
    Close,
    Punct,
    Inner,
}

/// Style modifiers for math content.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum MathStyle {
    Bold,
    Italic,
    BoldItalic,
    Blackboard,
    Calligraphic,
    SansSerif,
    Monospace,
    Fraktur,
}

/// Delimiter types for `\left`, `\right`, etc.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum DelimKind {
    LParen,
    RParen,
    LBracket,
    RBracket,
    LBrace,
    RBrace,
    LFloor,
    RFloor,
    LCeil,
    RCeil,
    LAngle,
    RAngle,
    Vert,
    DoubleVert,
    Dot,
}

// ── Shared helper types ────────────────────────────────────────

/// An argument to a LaTeX command (required `{}` or optional `[]`).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Arg {
    pub kind: ArgKind,
    pub content: ArgContent,
    pub span: Span,
}

/// Whether an argument is required (`{}`) or optional (`[]`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ArgKind {
    Required,
    Optional,
}

/// The content inside an argument: either parsed inlines or raw text.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ArgContent {
    Inlines(Vec<Inline>),
    Raw(String),
}
