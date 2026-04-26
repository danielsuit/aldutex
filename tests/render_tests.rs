use aldutex;

#[test]
fn test_section_body_not_dropped() {
    let source = r#"\documentclass{article}
\begin{document}
\section{Intro}
This is body text that should render.
\subsection{Sub}
More body text.
\end{document}"#;

    let (pdf, diag) = aldutex::compile(source);
    
    assert!(!diag.has_errors(), "Diagnostics should have no errors: {:?}", diag.errors);
    let pdf = pdf.expect("PDF should be generated");
    
    std::fs::write("debug_output.pdf", &pdf).unwrap();

    // Check for expected strings in the PDF binary
    assert!(contains_bytes(&pdf, b"Intro"), "PDF should contain 'Intro'");
    assert!(contains_bytes(&pdf, b"body text"), "PDF should contain 'body text'");
    assert!(contains_bytes(&pdf, b"Sub"), "PDF should contain 'Sub'");
    assert!(contains_bytes(&pdf, b"More body text"), "PDF should contain 'More body text'");
}

#[test]
fn test_section_layout_not_dropped() {
    let source = r#"\documentclass{article}
\begin{document}
\section{Intro}
This is body text that should render.
\subsection{Sub}
More body text.
\end{document}"#;

    let tokens = aldutex::lexer::Lexer::new(source).tokenize();
    let (doc, diag) = aldutex::parser::Parser::new(tokens, source).parse();
    assert!(!diag.has_errors());

    let fonts = aldutex::fonts::loader::FontRegistry::new().unwrap();
    let layout = aldutex::layout::page::PageLayout::letter_default();
    let pages = aldutex::layout::page::layout_document(&doc, &fonts, &layout);

    assert_eq!(pages.len(), 1);
    // 1 (Intro) + 1 (Paragraph 1) + 1 (Sub) + 1 (Paragraph 2) = 4 lines total expected
    assert_eq!(pages[0].lines.len(), 4, "Should have 4 lines, but has {}", pages[0].lines.len());
}

fn contains_bytes(haystack: &[u8], needle: &[u8]) -> bool {
    haystack.windows(needle.len()).any(|w| w == needle)
}
