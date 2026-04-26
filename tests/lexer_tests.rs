//! Lexer tests for the Aldutex LaTeX lexer.

use aldutex::lexer::{Lexer, Token, TokenKind};

fn lex(input: &str) -> Vec<Token> {
    Lexer::new(input).tokenize()
}

fn kinds(input: &str) -> Vec<TokenKind> {
    lex(input).into_iter().map(|t| t.kind).collect()
}

#[test]
fn test_simple_word() {
    let tokens = kinds("hello");
    assert_eq!(tokens, vec![TokenKind::Word("hello".into())]);
}

#[test]
fn test_multiple_words() {
    let tokens = kinds("hello world");
    assert_eq!(
        tokens,
        vec![
            TokenKind::Word("hello".into()),
            TokenKind::Whitespace,
            TokenKind::Word("world".into()),
        ]
    );
}

#[test]
fn test_command() {
    let tokens = kinds("\\textbf");
    assert_eq!(tokens, vec![TokenKind::Command("textbf".into())]);
}

#[test]
fn test_command_with_arg() {
    let tokens = kinds("\\command{arg}");
    assert_eq!(
        tokens,
        vec![
            TokenKind::Command("command".into()),
            TokenKind::BeginGroup,
            TokenKind::Word("arg".into()),
            TokenKind::EndGroup,
        ]
    );
}

#[test]
fn test_comment() {
    let tokens = kinds("Hello % comment\nWorld");
    assert_eq!(
        tokens,
        vec![
            TokenKind::Word("Hello".into()),
            TokenKind::Whitespace,
            TokenKind::Newline,
            TokenKind::Word("World".into()),
        ]
    );
}

#[test]
fn test_display_math() {
    let tokens = kinds("$$x^2$$");
    assert_eq!(
        tokens,
        vec![
            TokenKind::DollarDollar,
            TokenKind::Word("x".into()),
            TokenKind::Caret,
            TokenKind::Number("2".into()),
            TokenKind::DollarDollar,
        ]
    );
}

#[test]
fn test_inline_math() {
    let tokens = kinds("$x$");
    assert_eq!(
        tokens,
        vec![
            TokenKind::Dollar,
            TokenKind::Word("x".into()),
            TokenKind::Dollar,
        ]
    );
}

#[test]
fn test_tilde() {
    let tokens = kinds("~");
    assert_eq!(tokens, vec![TokenKind::Tilde]);
}

#[test]
fn test_multiple_spaces_collapse() {
    let tokens = kinds("a   b");
    assert_eq!(
        tokens,
        vec![
            TokenKind::Word("a".into()),
            TokenKind::Whitespace,
            TokenKind::Word("b".into()),
        ]
    );
}

#[test]
fn test_paragraph_break() {
    let tokens = kinds("a\n\n\nb");
    assert_eq!(
        tokens,
        vec![
            TokenKind::Word("a".into()),
            TokenKind::ParagraphBreak,
            TokenKind::Word("b".into()),
        ]
    );
}

#[test]
fn test_single_newline() {
    let tokens = kinds("a\nb");
    assert_eq!(
        tokens,
        vec![
            TokenKind::Word("a".into()),
            TokenKind::Newline,
            TokenKind::Word("b".into()),
        ]
    );
}

#[test]
fn test_single_char_command() {
    let tokens = kinds("\\,");
    assert_eq!(tokens, vec![TokenKind::Command(",".into())]);
}

#[test]
fn test_backslash_linebreak() {
    let tokens = kinds("\\\\");
    assert_eq!(tokens, vec![TokenKind::Backslash]);
}

#[test]
fn test_braces() {
    let tokens = kinds("{}");
    assert_eq!(tokens, vec![TokenKind::BeginGroup, TokenKind::EndGroup]);
}

#[test]
fn test_brackets() {
    let tokens = kinds("[]");
    assert_eq!(
        tokens,
        vec![TokenKind::BracketOpen, TokenKind::BracketClose]
    );
}

#[test]
fn test_ampersand() {
    let tokens = kinds("&");
    assert_eq!(tokens, vec![TokenKind::Ampersand]);
}

#[test]
fn test_caret() {
    let tokens = kinds("^");
    assert_eq!(tokens, vec![TokenKind::Caret]);
}

#[test]
fn test_underscore() {
    let tokens = kinds("_");
    assert_eq!(tokens, vec![TokenKind::Underscore]);
}

#[test]
fn test_hash() {
    let tokens = kinds("#");
    assert_eq!(tokens, vec![TokenKind::Hash]);
}

#[test]
fn test_at() {
    let tokens = kinds("@");
    assert_eq!(tokens, vec![TokenKind::At]);
}

#[test]
fn test_number() {
    let tokens = kinds("42");
    assert_eq!(tokens, vec![TokenKind::Number("42".into())]);
}

#[test]
fn test_decimal_number() {
    let tokens = kinds("3.14");
    assert_eq!(tokens, vec![TokenKind::Number("3.14".into())]);
}

#[test]
fn test_special_char() {
    let tokens = kinds("(");
    assert_eq!(tokens, vec![TokenKind::Special('(')]);
}

#[test]
fn test_begin_document() {
    let tokens = kinds("\\begin{document}");
    assert_eq!(
        tokens,
        vec![
            TokenKind::Command("begin".into()),
            TokenKind::BeginGroup,
            TokenKind::Word("document".into()),
            TokenKind::EndGroup,
        ]
    );
}

#[test]
fn test_mixed_content() {
    let tokens = kinds("Hello $x^2$ world");
    assert_eq!(
        tokens,
        vec![
            TokenKind::Word("Hello".into()),
            TokenKind::Whitespace,
            TokenKind::Dollar,
            TokenKind::Word("x".into()),
            TokenKind::Caret,
            TokenKind::Number("2".into()),
            TokenKind::Dollar,
            TokenKind::Whitespace,
            TokenKind::Word("world".into()),
        ]
    );
}

#[test]
fn test_escaped_percent() {
    let tokens = kinds("\\%");
    assert_eq!(tokens, vec![TokenKind::Command("%".into())]);
}

#[test]
fn test_empty_input() {
    let tokens = kinds("");
    assert!(tokens.is_empty());
}

#[test]
fn test_whitespace_only() {
    let tokens = kinds("   ");
    assert_eq!(tokens, vec![TokenKind::Whitespace]);
}

#[test]
fn test_paragraph_break_with_spaces() {
    let tokens = kinds("a\n  \n  b");
    assert_eq!(
        tokens,
        vec![
            TokenKind::Word("a".into()),
            TokenKind::ParagraphBreak,
            TokenKind::Word("b".into()),
        ]
    );
}
