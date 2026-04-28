use aldutex::ast;
use aldutex::lexer::Lexer;
use aldutex::parser::Parser;

fn parse_source(source: &str) -> (ast::Document, aldutex::error::Diagnostics) {
	let tokens = Lexer::new(source).tokenize();
	Parser::new(tokens, source).parse()
}

#[test]
fn test_math_symbols_parse_without_errors() {
	let source = r#"\documentclass{article}
\begin{document}
$$\alpha + \beta = \gamma \leq \delta \times \infty$$
$$\forall x \in A,\ \exists y \notin B$$
$$\rightarrow\ \Leftarrow\ \iff\ \approx\ \neq\ \subseteq\ \supseteq$$
$$\sin(x) + \cos(y) + \tan(z) + \log t + \lim_{n\to\infty} a_n$$
$$\sum_{i=1}^{n} i\quad\prod_{k=1}^{m} k\quad\int_0^1 x^2$$
$$\frac{-b + \sqrt{b^2 - 4ac}}{2a}$$
\end{document}"#;

	let (_doc, diag) = parse_source(source);
	assert!(!diag.has_errors(), "Errors: {:?}", diag.errors);
}

#[test]
fn test_math_symbols_layout_has_visible_glyphs() {
	let source = r#"\documentclass{article}
\begin{document}
Symbols: $$\alpha + \beta = \gamma\ \times\ \sum_{i=1}^{n} i\ \rightarrow\ \Omega$$
Functions: $$\sin(x)+\cos(y)+\log z+\lim_{n\to\infty} a_n$$
Complex: $$\frac{-b + \sqrt{b^2 - 4ac}}{2a}$$
\end{document}"#;

	let tokens = Lexer::new(source).tokenize();
	let (doc, diag) = Parser::new(tokens, source).parse();
	assert!(!diag.has_errors(), "Errors: {:?}", diag.errors);

	let fonts = aldutex::fonts::loader::FontRegistry::new().unwrap();
	let layout = aldutex::layout::page::PageLayout::letter_default();
	let pages = aldutex::layout::page::layout_document(&doc, &fonts, &layout);

	assert!(!pages.is_empty(), "Expected at least one laid out page");

	let glyph_count = pages
		.iter()
		.flat_map(|p| p.lines.iter())
		.flat_map(|l| l.boxes.iter())
		.filter(|b| matches!(b.content, aldutex::layout::boxes::BoxContent::Glyph { .. }))
		.count();

	assert!(
		glyph_count > 40,
		"Expected many rendered glyphs for broad symbol coverage, got {glyph_count}"
	);
}
