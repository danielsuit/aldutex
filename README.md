# Aldutex

Aldutex is a pure-Rust LaTeX typesetting engine that turns `.tex` source into PDF bytes.

This repository is currently a library crate, not a full TeX distribution and not a CLI tool. The core idea is:

1. tokenize a LaTeX-like source string,
2. parse it into an AST,
3. shape text with bundled fonts,
4. lay the document out into positioned boxes,
5. render those boxes into a PDF.

It is already useful as a compact typesetting pipeline, especially for paragraphs, sections, lists, and a growing subset of math, but it is still intentionally smaller than full LaTeX.

## What This Project Is

Aldutex is best thought of as a document compiler with a TeX-inspired front end and a custom Rust layout/rendering back end.

- It parses a subset of LaTeX syntax into structured Rust data.
- It uses OpenType shaping and font metrics directly instead of shelling out to TeX.
- It lays text out with a Knuth-Plass paragraph breaker.
- It renders PDF output directly from positioned glyphs, rules, and paths.
- It includes an incremental compilation mode that reuses block layout work across recompiles.

What it is not:

- not a drop-in replacement for `pdflatex`,
- not a macro-expansion engine,
- not a complete implementation of all LaTeX packages or environments,
- not currently a command-line application.

## Current State

The parser understands more than the renderer currently draws, so it helps to separate "parsed" from "fully laid out and rendered".

| Area | Status | Notes |
| --- | --- | --- |
| Basic document parsing | Good | Preamble, body blocks, inline content, and math AST are implemented. |
| Paragraph layout | Good | Text is shaped into glyphs and broken with Knuth-Plass. |
| Sections | Good | Section headings are laid out and section bodies recurse normally. |
| Lists | Good | `itemize`, `enumerate`, and `description` are laid out with markers. |
| Inline math | Good, subset | Fractions, roots, scripts, large operators, delimiters, arrows, Greek, and common symbols are supported. |
| Display math | Good, subset | `$$...$$`, `\[...\]`, and math environments map to display layout. |
| Diagnostics | Good | Errors and warnings carry source spans through `miette`. |
| Incremental compile | Working | Reuses cached line layout for unchanged top-level blocks. |
| Tables / figures / verbatim | Parsed, not fully rendered | The parser builds AST nodes, but document layout/rendering is not complete for these blocks yet. |
| Links / citations / refs / footnotes | Parsed, partially wired | The parser recognizes them, but full resolution/rendering is still incomplete. |
| Full LaTeX compatibility | Not a goal yet | This is a focused engine, not a full macro processor. |

## High-Level Architecture

The core pipeline in `src/lib.rs` looks like this:

```text
source text
  -> lexer::Lexer
  -> Vec<Token>
  -> parser::Parser
  -> ast::Document + Diagnostics
  -> fonts::loader::FontRegistry
  -> layout::page::layout_document
  -> Vec<LayoutPage>
  -> renderer::pdf::render_to_pdf
  -> PDF bytes
```

The public API exposes three main entry points:

- `aldutex::compile(source)` for full parse + layout + PDF output,
- `aldutex::compile_incremental(source, &mut cache)` for cached recompiles,
- `aldutex::parse(source)` for AST + diagnostics only.

## How Compilation Works

### 1. Lexing

Code: `src/lexer.rs`

The lexer is a single-pass tokenizer over the source string. It emits tokens such as:

- words,
- whitespace, newlines, and paragraph breaks,
- commands like `\textbf` or `\section`,
- group delimiters `{...}` and `[...]`,
- math delimiters like `$` and `$$`,
- special tokens such as `^`, `_`, `~`, `&`, `\\`, and numeric literals.

This stage does not try to understand document structure. Its job is to preserve enough information for the parser, including source spans.

### 2. Parsing

Code: `src/parser.rs`, `src/ast.rs`

The parser is a recursive-descent parser that turns the token stream into a typed AST.

At the top level it splits the document into:

- a preamble,
- a body made of block nodes,
- per-node source spans for diagnostics and caching.

#### Preamble parsing

The preamble parser recognizes:

- `\documentclass`,
- `\usepackage`,
- `\title`,
- `\author`,
- `\date`,
- the transition into `\begin{document}`.

At the moment, the most visible layout effect from the preamble is page size selection: `a4paper` switches to A4; otherwise the engine defaults to letter-sized pages.

#### Block parsing

The body parser can build AST nodes for blocks such as:

- paragraphs,
- sectioning commands (`\section`, `\subsection`, and related levels),
- lists (`itemize`, `enumerate`, `description`),
- display math,
- `\vspace`, `\newpage`, `\clearpage`, `\hline`, `\hrule`,
- figures, tables, tabular environments,
- verbatim-like environments,
- raw commands it chooses to preserve instead of dropping.

#### Inline parsing

Inside paragraphs, the parser recognizes inline constructs such as:

- plain text,
- `\textbf`, `\textit`, `\texttt`, `\textsc`, `\emph`, `\underline`,
- inline math,
- links (`\href`, `\url`),
- references and citations,
- footnotes,
- spacing commands like `~`, `\hspace`, `\quad`, `\qquad`,
- a handful of text command shorthands and symbol substitutions.

#### Math parsing

Math is parsed into a separate tree of `MathNode` values. Supported node kinds include:

- atoms and identifiers,
- numbers,
- groups and rows,
- superscripts and subscripts,
- fractions (`\frac`),
- square roots (`\sqrt`),
- large operators like `\sum`, `\prod`, `\int`, `\iint`, `\iiint`,
- operator names like `\sin`, `\log`, `\lim`,
- styled math like `\mathbf`, `\mathit`, `\mathbb`, `\mathcal`,
- overs and unders,
- `\left ... \right` delimiters,
- a broad set of Greek letters, arrows, binary operators, relations, and other symbols.

The parser does not stop at the first problem. It records diagnostics and keeps going where possible.

### 3. Diagnostics

Code: `src/error.rs`

Diagnostics are accumulated in a `Diagnostics` struct with separate `errors` and `warnings` lists.

Important details:

- spans are tracked with `Span { start, end }`,
- parse errors are reported with source locations,
- unknown commands can become warnings instead of fatal errors,
- font and render failures are also converted into diagnostics by the top-level compile functions.

This makes the crate usable in editor-like workflows where partial feedback is more useful than fail-fast behavior.

### 4. Fonts, Shaping, and Metrics

Code: `src/fonts/mod.rs`, `src/fonts/loader.rs`, `src/fonts/shaper.rs`, `src/fonts/metrics.rs`, `src/fonts/math_font.rs`

Aldutex embeds its fonts directly into the binary at compile time.

Bundled fonts:

- Latin Modern Roman Regular
- Latin Modern Roman Bold
- Latin Modern Roman Italic
- Latin Modern Roman Bold Italic
- Latin Modern Mono
- Latin Modern Math

The font subsystem does three separate jobs:

#### Loading

`FontRegistry::new()` loads the bundled font bytes and exposes them by `FontId`.

#### Shaping

`src/fonts/shaper.rs` uses `rustybuzz` to shape strings into positioned glyphs. This is where:

- glyph IDs are chosen,
- kerning and OpenType shaping happen,
- glyph advances and offsets are scaled into points.

#### Metrics

`src/fonts/metrics.rs` uses `skrifa` to query:

- ascenders,
- descenders,
- glyph bounding boxes,
- advance widths,
- space widths.

#### Math constants

`src/fonts/math_font.rs` reads OpenType MATH table constants with `ttf-parser`. Those constants influence:

- fraction rule thickness,
- numerator and denominator shifts,
- radical rule thickness,
- radical gaps,
- superscript and subscript positioning,
- axis height and other math spacing values.

If a font does not provide the expected MATH data, the engine falls back to hard-coded defaults.

### 5. Layout

Code: `src/layout/page.rs`, `src/layout/paragraph.rs`, `src/layout/math.rs`, `src/layout/boxes.rs`

Layout is where the AST becomes positioned boxes.

#### The box model

Everything is eventually expressed as `LayoutBox` values. A box can hold:

- a glyph,
- a rectangular rule,
- a custom path,
- an image,
- a link wrapper.

Lines are stored as `LayoutLine`, and full pages as `LayoutPage`.

#### Page geometry

`PageLayout` stores page width, height, and margins in points.

- `letter_default()` is the fallback.
- `a4_default()` is used when `a4paper` is present in the document class options.

#### Paragraph layout

For paragraph blocks, the engine:

1. walks the inline AST,
2. shapes text into glyph boxes,
3. turns spaces into Knuth-Plass glue,
4. inserts penalties for legal break points,
5. runs `break_paragraph(...)`,
6. converts the chosen breakpoints into `LayoutLine` values.

This is the most TeX-like part of the engine: it aims to choose globally better line breaks rather than greedily filling one line at a time.

#### Section layout

Sections are laid out as bold heading lines at larger sizes, then their nested body blocks are laid out recursively below them.

#### List layout

Lists are handled explicitly in page layout:

- `itemize` gets bullet markers,
- `enumerate` gets numeric markers,
- `description` uses the label text as the marker column.

Nested lists are indented by recursively calling the same list layout helper with a deeper offset.

#### Math layout

Math layout is currently a custom recursive box builder.

Examples of how it works:

- identifiers and symbols become math glyph boxes,
- superscripts/subscripts are laid out at a reduced size and vertically shifted,
- fractions are built from numerator boxes, a rule box, and denominator boxes,
- square roots are built from a custom path plus the radicand layout,
- display math is centered inside the text column,
- large operators and named operators are mapped from parser nodes into math atoms/operators.

One useful implementation detail: math coordinates are stored in a baseline-relative space where positive `y` means "up". The PDF renderer later converts that into page coordinates.

### 6. Pagination

Code: `src/layout/page.rs`

After a block has been laid out into lines, those lines are placed on pages.

The paginator:

- tracks the current vertical position,
- starts a new page when a line would overflow the bottom margin,
- offsets each line by the page margins,
- stores the final baseline position in each `LayoutLine`.

This stage is intentionally simple right now. It does not yet do sophisticated widow/orphan control, float placement, or advanced footnote balancing.

### 7. PDF Rendering

Code: `src/renderer/pdf.rs`

The renderer walks each page and emits PDF drawing commands using `krilla`.

Glyph rendering works like this:

- consecutive glyph boxes with the same font and size are grouped into runs,
- each run is written with the matching embedded font,
- per-glyph offsets are preserved inside the run.

Non-glyph content is rendered separately:

- `Rule` boxes become filled rectangles,
- `Path` boxes become filled polygons,
- those are especially important for fraction bars, radicals, and other math constructs.

At the end, the `krilla::Document` is finalized into a `Vec<u8>`.

## Incremental Compilation

Code: `src/cache.rs`

`compile_incremental(...)` still lexes and reparses the full source, but it can skip some layout work for unchanged top-level blocks.

The cache works by:

1. taking each top-level block's source span,
2. hashing the raw source slice for that block,
3. reusing previously laid out `Vec<LayoutLine>` when the hash matches,
4. repaginating the reused lines into fresh pages.

That means the cache is:

- block-level, not token-level,
- layout-focused, not parse-focused,
- good for editor loops where a few blocks change at a time.

It also means pagination can still shift even when a block is reused, because page assembly is recomputed after line reuse.

## Public API Example

### Full compile

```rust
let source = r#"\documentclass{article}
\begin{document}
Hello, world!
\[
\frac{-b \pm \sqrt{b^2 - 4ac}}{2a}
\]
\end{document}"#;

let (pdf, diagnostics) = aldutex::compile(source);

if diagnostics.has_errors() {
    for err in &diagnostics.errors {
        eprintln!("{err}");
    }
}

if let Some(bytes) = pdf {
    std::fs::write("output.pdf", bytes).unwrap();
}
```

### Parse only

```rust
let (doc, diagnostics) = aldutex::parse(r"\section{Intro}Hello");
println!("top-level blocks: {}", doc.body.len());
println!("errors: {}", diagnostics.errors.len());
```

### Incremental compile

```rust
let mut cache = aldutex::new_cache();

let first = r"\begin{document}Hello\end{document}";
let second = r"\begin{document}Hello again\end{document}";

let (_pdf1, _diag1) = aldutex::compile_incremental(first, &mut cache);
let (_pdf2, _diag2) = aldutex::compile_incremental(second, &mut cache);
```

## Repository Map

Here is the quickest way to navigate the codebase:

- `src/lib.rs` - public API and compile pipeline
- `src/lexer.rs` - source text to tokens
- `src/parser.rs` - tokens to AST
- `src/ast.rs` - document, block, inline, and math node types
- `src/error.rs` - spans and diagnostics
- `src/fonts/` - font loading, shaping, metrics, math constants
- `src/layout/boxes.rs` - low-level layout box model
- `src/layout/paragraph.rs` - Knuth-Plass implementation
- `src/layout/page.rs` - document and page layout
- `src/layout/math.rs` - recursive math box layout
- `src/cache.rs` - incremental compilation cache
- `src/renderer/pdf.rs` - PDF output
- `tests/` - parser, lexer, layout, render, and math coverage

## Running the Project

This repo is currently a library crate, so the usual development entry point is tests:

```bash
cargo test
```

There are a few especially useful test groups:

- `tests/lexer_tests.rs` checks tokenization behavior,
- `tests/parser_tests.rs` checks AST shape and diagnostics,
- `tests/layout_tests.rs` checks line-breaking behavior,
- `tests/render_tests.rs` checks rendered PDF content and debug output,
- `tests/math_tests.rs` checks broad math parsing and visible glyph coverage.

## Limitations and Honest Caveats

If you are reading the code to extend it, these are the main constraints to know up front:

- It is a LaTeX subset, not a full macro engine.
- Some nodes are parsed before they are fully laid out or rendered.
- Cross-references, citations, and footnotes are not fully resolved end to end yet.
- Table, figure, image, and verbatim support are not complete in the final layout/render pipeline.
- The renderer is PDF-only right now.
- Font selection is currently built around bundled Latin Modern fonts.
- The line breaker is implemented, but higher-level page composition is still intentionally simple.

## Why The Design Looks This Way

This codebase is organized around a clean separation of concerns:

- lexing and parsing are syntax-focused,
- layout is geometry-focused,
- rendering is output-focused,
- diagnostics and spans flow through the whole system,
- incremental compilation is isolated in its own cache layer.

That separation makes the engine easier to test, easier to embed into other tools, and easier to grow without coupling parsing decisions directly to PDF drawing code.

If you want to start exploring the implementation, the best order is:

1. `src/lib.rs`
2. `src/ast.rs`
3. `src/parser.rs`
4. `src/layout/page.rs`
5. `src/layout/paragraph.rs`
6. `src/layout/math.rs`
7. `src/renderer/pdf.rs`
