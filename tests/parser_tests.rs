//! Parser tests for the Aldutex LaTeX parser.

use aldutex::ast::*;
use aldutex::error::Diagnostics;
use aldutex::lexer::Lexer;
use aldutex::parser::Parser;

fn parse_source(source: &str) -> (Document, Diagnostics) {
    let tokens = Lexer::new(source).tokenize();
    Parser::new(tokens, source).parse()
}

#[test]
fn test_basic_text_parse() {
    let source = include_str!("golden/basic_text.tex");
    let (doc, diag) = parse_source(source);
    assert!(!diag.has_errors(), "Errors: {:?}", diag.errors);
    assert_eq!(doc.preamble.document_class.name, "article");
    assert!(!doc.body.is_empty(), "Body should not be empty");
}

#[test]
fn test_sections_parse() {
    let source = include_str!("golden/sections.tex");
    let (doc, diag) = parse_source(source);
    assert!(!diag.has_errors(), "Errors: {:?}", diag.errors);

    // Should have sections in the body
    let has_section = doc.body.iter().any(|b| matches!(b, Block::Section { .. }));
    assert!(has_section, "Should contain at least one Section block");
}

#[test]
fn test_section_with_label() {
    let source = r#"\documentclass{article}
\begin{document}
\section{Intro}\label{sec:intro}
Hello.
\end{document}"#;
    let (doc, diag) = parse_source(source);
    assert!(!diag.has_errors(), "Errors: {:?}", diag.errors);

    if let Some(Block::Section { label, level, .. }) = doc.body.first() {
        assert_eq!(*level, 1);
        assert_eq!(label.as_deref(), Some("sec:intro"));
    } else {
        panic!("Expected a Section block, got: {:?}", doc.body.first());
    }
}

#[test]
fn test_nested_bold_italic() {
    let source = r#"\documentclass{article}
\begin{document}
\textbf{\textit{x}}
\end{document}"#;
    let (doc, diag) = parse_source(source);
    assert!(!diag.has_errors(), "Errors: {:?}", diag.errors);

    if let Some(Block::Paragraph { inlines, .. }) = doc.body.first() {
        match inlines.first() {
            Some(Inline::Bold { content, .. }) => {
                assert!(
                    matches!(content.first(), Some(Inline::Italic { .. })),
                    "Expected Italic inside Bold, got: {:?}",
                    content.first()
                );
            }
            other => panic!("Expected Bold, got: {:?}", other),
        }
    } else {
        panic!("Expected Paragraph, got: {:?}", doc.body.first());
    }
}

#[test]
fn test_inline_math() {
    let source = r#"\documentclass{article}
\begin{document}
$x^2 + y^2$
\end{document}"#;
    let (doc, diag) = parse_source(source);
    assert!(!diag.has_errors(), "Errors: {:?}", diag.errors);

    if let Some(Block::Paragraph { inlines, .. }) = doc.body.first() {
        let has_math = inlines.iter().any(|i| matches!(i, Inline::Math { .. }));
        assert!(has_math, "Should contain inline Math node");
    } else {
        panic!("Expected Paragraph");
    }
}

#[test]
fn test_itemize_list() {
    let source = r#"\documentclass{article}
\begin{document}
\begin{itemize}
\item a
\item b
\end{itemize}
\end{document}"#;
    let (doc, diag) = parse_source(source);
    assert!(!diag.has_errors(), "Errors: {:?}", diag.errors);

    if let Some(Block::List { kind, items, .. }) = doc.body.first() {
        assert_eq!(*kind, ListKind::Itemize);
        assert_eq!(items.len(), 2);
    } else {
        panic!("Expected List, got: {:?}", doc.body.first());
    }
}

#[test]
fn test_unclosed_begin() {
    let source = r#"\documentclass{article}
\begin{document}
\begin{itemize}
\item hello
\end{document}"#;
    let (_doc, diag) = parse_source(source);
    assert!(
        diag.has_errors(),
        "Should have error for unclosed environment"
    );
}

#[test]
fn test_unknown_command_warning() {
    let source = r#"\documentclass{article}
\begin{document}
\foobar{test}
\end{document}"#;
    let (_doc, diag) = parse_source(source);
    assert!(
        !diag.warnings.is_empty(),
        "Should have warning for unknown command"
    );
}

#[test]
fn test_footnote() {
    let source = r#"\documentclass{article}
\begin{document}
Hello\footnote{A note}.
\end{document}"#;
    let (doc, diag) = parse_source(source);
    assert!(!diag.has_errors(), "Errors: {:?}", diag.errors);

    if let Some(Block::Paragraph { inlines, .. }) = doc.body.first() {
        let has_footnote = inlines
            .iter()
            .any(|i| matches!(i, Inline::FootnoteRef { .. }));
        assert!(has_footnote, "Should contain FootnoteRef");
    }
}

#[test]
fn test_tabular() {
    let source = r#"\documentclass{article}
\begin{document}
\begin{tabular}{lcr}
a & b & c \\
\end{tabular}
\end{document}"#;
    let (doc, diag) = parse_source(source);
    assert!(!diag.has_errors(), "Errors: {:?}", diag.errors);

    let has_table = doc.body.iter().any(|b| matches!(b, Block::Table { .. }));
    assert!(has_table, "Should contain a Table block");
}

#[test]
fn test_math_inline_golden() {
    let source = include_str!("golden/math_inline.tex");
    let (doc, diag) = parse_source(source);
    assert!(!diag.has_errors(), "Errors: {:?}", diag.errors);
    assert!(!doc.body.is_empty());
}

#[test]
fn test_math_display_golden() {
    let source = include_str!("golden/math_display.tex");
    let (_doc, diag) = parse_source(source);
    assert!(!diag.has_errors(), "Errors: {:?}", diag.errors);
}

#[test]
fn test_table_golden() {
    let source = include_str!("golden/table.tex");
    let (_doc, diag) = parse_source(source);
    assert!(!diag.has_errors(), "Errors: {:?}", diag.errors);
}

#[test]
fn test_itemize_golden() {
    let source = include_str!("golden/itemize.tex");
    let (_doc, diag) = parse_source(source);
    assert!(!diag.has_errors(), "Errors: {:?}", diag.errors);
}

#[test]
fn test_em_dash() {
    let source = r#"\documentclass{article}
\begin{document}
Hello---world.
\end{document}"#;
    let (doc, diag) = parse_source(source);
    assert!(!diag.has_errors(), "Errors: {:?}", diag.errors);

    if let Some(Block::Paragraph { inlines, .. }) = doc.body.first() {
        let has_emdash = inlines
            .iter()
            .any(|i| matches!(i, Inline::Text { content, .. } if content.contains('\u{2014}')));
        assert!(has_emdash, "Should contain em-dash character");
    }
}

#[test]
fn test_nonbreaking_space() {
    let source = r#"\documentclass{article}
\begin{document}
Hello~world.
\end{document}"#;
    let (doc, diag) = parse_source(source);
    assert!(!diag.has_errors(), "Errors: {:?}", diag.errors);

    if let Some(Block::Paragraph { inlines, .. }) = doc.body.first() {
        let has_nbsp = inlines
            .iter()
            .any(|i| matches!(i, Inline::NonBreakingSpace { .. }));
        assert!(has_nbsp, "Should contain NonBreakingSpace");
    }
}
#[test]
fn test_display_math_brackets() {
    let source = r#"\documentclass{article}
\begin{document}
\[ x^2 + 1 = 0 \]
\end{document}"#;
    let (doc, diag) = parse_source(source);
    assert!(!diag.has_errors(), "Errors: {:?}", diag.errors);
    assert!(diag.warnings.is_empty(), "Warnings: {:?}", diag.warnings);

    if let Some(Block::Paragraph { inlines, .. }) = doc.body.first() {
        let has_math = inlines.iter().any(|i| matches!(i, Inline::Math { .. }));
        assert!(has_math, "Should contain Math node from \\[ \\]");
    } else {
        panic!("Expected Paragraph with Math node");
    }
}
