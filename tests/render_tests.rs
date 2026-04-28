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

#[test]
fn test_math_layout_not_dropped() {
    let source = r#"\documentclass{article}
\begin{document}
Math: $x + 1$
\end{document}"#;

    let tokens = aldutex::lexer::Lexer::new(source).tokenize();
    let (doc, diag) = aldutex::parser::Parser::new(tokens, source).parse();
    assert!(!diag.has_errors());

    let fonts = aldutex::fonts::loader::FontRegistry::new().unwrap();
    let layout = aldutex::layout::page::PageLayout::letter_default();
    let pages = aldutex::layout::page::layout_document(&doc, &fonts, &layout);

    assert_eq!(pages.len(), 1);
    // "Math:" is separate, and then "$x + 1$" follows
    // With current layout, "Math: $x + 1$" should all be on one line if they fit
    assert!(pages[0].lines.len() >= 1);
    
    // Check if any line contains glyphs from the math font (FontId 5)
    let has_math_glyph = pages[0].lines.iter().any(|line| {
        line.boxes.iter().any(|b| {
            if let aldutex::layout::boxes::BoxContent::Glyph { font_id, .. } = &b.content {
                font_id.0 == 5
            } else {
                false
            }
        })
    });
    
    assert!(has_math_glyph, "Layout should contain math glyphs from FontId 5");
}

#[test]
fn test_math_layout_double_dollar_not_dropped() {
    let source = r#"\documentclass{article}
\begin{document}
Math: $$x + 1$$
\end{document}"#;

    let (pdf, compile_diag) = aldutex::compile(source);
    assert!(
        !compile_diag.has_errors(),
        "Diagnostics should have no errors: {:?}",
        compile_diag.errors
    );
    let _pdf = pdf.expect("PDF should be generated");

    let tokens = aldutex::lexer::Lexer::new(source).tokenize();
    let (doc, diag) = aldutex::parser::Parser::new(tokens, source).parse();
    assert!(!diag.has_errors());

    let fonts = aldutex::fonts::loader::FontRegistry::new().unwrap();
    let layout = aldutex::layout::page::PageLayout::letter_default();
    let pages = aldutex::layout::page::layout_document(&doc, &fonts, &layout);

    assert_eq!(pages.len(), 1);
    assert!(pages[0].lines.len() >= 1);

    let has_math_glyph = pages[0].lines.iter().any(|line| {
        line.boxes.iter().any(|b| {
            if let aldutex::layout::boxes::BoxContent::Glyph { font_id, .. } = &b.content {
                font_id.0 == 5
            } else {
                false
            }
        })
    });

    assert!(
        has_math_glyph,
        "Layout should contain math glyphs from inline $$...$$"
    );
}

#[test]
fn test_itemize_renders_bullets_and_text() {
    let source = r#"\documentclass{article}
\begin{document}
Before list.
\begin{itemize}
\item First apple item.
\item Second banana item.
\item Third cherry item.
\end{itemize}
After list.
\end{document}"#;

    let tokens = aldutex::lexer::Lexer::new(source).tokenize();
    let (doc, diag) = aldutex::parser::Parser::new(tokens, source).parse();
    assert!(!diag.has_errors(), "Diagnostics: {:?}", diag.errors);

    let fonts = aldutex::fonts::loader::FontRegistry::new().unwrap();
    let layout = aldutex::layout::page::PageLayout::letter_default();
    let pages = aldutex::layout::page::layout_document(&doc, &fonts, &layout);

    let total_lines: usize = pages.iter().map(|p| p.lines.len()).sum();
    // 1 (Before list.) + 3 (one per item) + 1 (After list.) = 5
    assert!(
        total_lines >= 5,
        "Expected >= 5 lines for itemize content, got {total_lines}"
    );

    let glyph_count: usize = pages
        .iter()
        .flat_map(|p| p.lines.iter())
        .flat_map(|l| l.boxes.iter())
        .filter(|b| matches!(
            b.content,
            aldutex::layout::boxes::BoxContent::Glyph { .. }
        ))
        .count();
    assert!(
        glyph_count > 40,
        "Expected list items to produce many glyphs, got {glyph_count}"
    );
}

#[test]
fn test_enumerate_numbers_each_item() {
    let source = r#"\documentclass{article}
\begin{document}
\begin{enumerate}
\item Alpha.
\item Beta.
\item Gamma.
\end{enumerate}
\end{document}"#;

    let tokens = aldutex::lexer::Lexer::new(source).tokenize();
    let (doc, diag) = aldutex::parser::Parser::new(tokens, source).parse();
    assert!(!diag.has_errors(), "Diagnostics: {:?}", diag.errors);

    let fonts = aldutex::fonts::loader::FontRegistry::new().unwrap();
    let layout = aldutex::layout::page::PageLayout::letter_default();
    let pages = aldutex::layout::page::layout_document(&doc, &fonts, &layout);

    let total_lines: usize = pages.iter().map(|p| p.lines.len()).sum();
    assert!(
        total_lines >= 3,
        "Expected one line per enumerate item, got {total_lines}"
    );
}

#[test]
fn test_supported_latex_symbol_debug_output_pdf() {
    let source = build_supported_symbol_debug_source();

    let (pdf, diag) = aldutex::compile(&source);
    assert!(!diag.has_errors(), "Diagnostics should have no errors: {:?}", diag.errors);
    let pdf = pdf.expect("PDF should be generated");
    std::fs::write("debug_output.pdf", &pdf).unwrap();

    let tokens = aldutex::lexer::Lexer::new(&source).tokenize();
    let (doc, parse_diag) = aldutex::parser::Parser::new(tokens, &source).parse();
    assert!(!parse_diag.has_errors());

    let fonts = aldutex::fonts::loader::FontRegistry::new().unwrap();
    let layout = aldutex::layout::page::PageLayout::letter_default();
    let pages = aldutex::layout::page::layout_document(&doc, &fonts, &layout);

    assert!(!pages.is_empty(), "Expected debug output to produce at least one page");

    let glyph_count = pages
        .iter()
        .flat_map(|page| page.lines.iter())
        .flat_map(|line| line.boxes.iter())
        .filter(|b| matches!(b.content, aldutex::layout::boxes::BoxContent::Glyph { .. }))
        .count();
    let shape_count = pages
        .iter()
        .flat_map(|page| page.lines.iter())
        .flat_map(|line| line.boxes.iter())
        .filter(|b| matches!(
            b.content,
            aldutex::layout::boxes::BoxContent::Rule { .. }
                | aldutex::layout::boxes::BoxContent::Path { .. }
        ))
        .count();
    assert!(
        glyph_count > 150,
        "Expected broad symbol coverage in debug output, got {glyph_count} glyphs"
    );
    assert!(
        shape_count >= 2,
        "Expected drawn shapes for fractions and square roots, got {shape_count}"
    );
}

fn contains_bytes(haystack: &[u8], needle: &[u8]) -> bool {
    haystack.windows(needle.len()).any(|w| w == needle)
}

fn build_supported_symbol_debug_source() -> String {
    let lowercase_greek = [
        "alpha", "beta", "gamma", "delta", "epsilon", "zeta", "eta", "theta", "iota",
        "kappa", "lambda", "mu", "nu", "xi", "pi", "rho", "sigma", "tau", "upsilon",
        "phi", "chi", "psi", "omega", "varepsilon", "vartheta", "varpi", "varrho",
        "varsigma", "varphi",
    ];
    let uppercase_greek = [
        "Gamma", "Delta", "Theta", "Lambda", "Xi", "Pi", "Sigma", "Upsilon", "Phi",
        "Psi", "Omega",
    ];
    let relations = [
        "leq", "geq", "neq", "approx", "equiv", "sim", "cong", "propto", "subset",
        "supset", "subseteq", "supseteq", "in", "notin", "ni", "forall", "exists",
        "nexists",
    ];
    let binary_ops = [
        "times", "div", "pm", "mp", "cdot", "circ", "bullet", "oplus", "otimes", "cup",
        "cap", "wedge", "vee", "setminus",
    ];
    let misc_symbols = [
        "infty", "nabla", "partial", "ell", "wp", "Re", "Im", "aleph", "hbar",
        "emptyset", "imath", "jmath",
    ];
    let arrows = [
        "rightarrow", "leftarrow", "Rightarrow", "Leftarrow", "leftrightarrow",
        "Leftrightarrow", "mapsto", "hookrightarrow", "hookleftarrow", "uparrow",
        "downarrow",
    ];
    let dots = ["ldots", "cdots", "vdots", "ddots"];

    let mut source = String::from(
        "\\documentclass{article}\n\\begin{document}\nSupported LaTeX math symbol coverage.\n\n",
    );
    push_display_math_block(
        &mut source,
        "Quadratic formula:",
        "\\frac{-b \\pm \\sqrt{b^2 - 4ac}}{2a} \\quad x^2 + y_1 + z_{i}^{n}",
    );
    push_symbol_block(&mut source, "Lowercase Greek:", &lowercase_greek);
    push_symbol_block(&mut source, "Uppercase Greek:", &uppercase_greek);
    push_symbol_block(&mut source, "Relations:", &relations);
    push_symbol_block(&mut source, "Binary operators:", &binary_ops);
    push_symbol_block(&mut source, "Misc symbols:", &misc_symbols);
    push_symbol_block(&mut source, "Arrows:", &arrows);
    push_symbol_block(&mut source, "Dots:", &dots);

    push_display_math_block(
        &mut source,
        "Large operators:",
        "\\sum_{i=1}^{n} i \\quad \\prod_{k=1}^{n} k \\quad \\coprod_{m=1}^{n} X_m",
    );
    push_display_math_block(
        &mut source,
        "",
        "\\bigcup_{i=1}^{n} A_i \\quad \\bigcap_{i=1}^{n} B_i \\quad \\bigoplus_{j=1}^{n} V_j \\quad \\bigotimes_{j=1}^{n} W_j",
    );
    push_display_math_block(
        &mut source,
        "",
        "\\int_0^1 x^2\\,dx \\quad \\oint_C f(z)\\,dz \\quad \\iint_R f(x,y)\\,dA \\quad \\iiint_V f(x,y,z)\\,dV",
    );

    push_display_math_block(
        &mut source,
        "Named operators:",
        "\\sin x \\cos x \\tan x \\cot x \\sec x \\csc x \\arcsin x \\arccos x \\arctan x",
    );
    push_display_math_block(
        &mut source,
        "",
        "\\sinh x \\cosh x \\tanh x \\log x \\ln x \\exp x \\lim_{n\\to\\infty} a_n \\limsup a_n \\liminf a_n",
    );
    push_display_math_block(
        &mut source,
        "",
        "\\sup A \\inf A \\min x \\max x \\det A \\gcd(a,b) \\dim V \\ker T \\hom(V,W) \\deg f \\arg z \\Pr(E) \\mod n",
    );

    push_display_math_block(
        &mut source,
        "Delimiters and structures:",
        "\\left( x \\right) \\left[ x \\right] \\left\\lbrace x \\right\\rbrace \\left| x \\right| \\left\\Vert x \\right\\Vert",
    );
    push_display_math_block(
        &mut source,
        "",
        "\\left\\langle x \\right\\rangle \\left\\lfloor x \\right\\rfloor \\left\\lceil x \\right\\rceil",
    );
    push_display_math_block(
        &mut source,
        "",
        "x^2 + y_1 + z_{i}^{n}",
    );

    source.push_str("\\end{document}");
    source
}

fn push_symbol_block(source: &mut String, label: &str, commands: &[&str]) {
    push_display_math_block(
        source,
        label,
        &commands
            .iter()
            .map(|command| format!("\\{command}"))
            .collect::<Vec<_>>()
            .join(" "),
    );
}

fn push_display_math_block(source: &mut String, label: &str, body: &str) {
    if !label.is_empty() {
        source.push_str(label);
        source.push_str("\n\n");
    }
    source.push_str("\\begin{displaymath}\n");
    source.push_str(body);
    source.push_str("\n\\end{displaymath}\n\n");
}
