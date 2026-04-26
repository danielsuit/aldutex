use aldutex;

#[test]
fn test_simple_pdf_contains_text() {
    let source = r#"\documentclass{article}
\begin{document}
HELLO
\end{document}"#;

    let (pdf, diag) = aldutex::compile(source);
    assert!(!diag.has_errors());
    let pdf = pdf.unwrap();
    
    // Write to a temporary file for manual inspection if needed
    // std::fs::write("test_output.pdf", &pdf).unwrap();

    assert!(pdf.windows(5).any(|w| w == b"HELLO"), "PDF should contain 'HELLO'");
}
