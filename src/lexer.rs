//! Lexer for LaTeX source code.
//!
//! Converts raw `&str` into `Vec<Token>`. Single-pass, stateless byte iterator.
//! The same input always produces the same output.

use crate::error::Span;
use serde::{Deserialize, Serialize};

/// A single token produced by the lexer.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Token {
    pub kind: TokenKind,
    pub span: Span,
}

/// The type of a lexer token.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum TokenKind {
    /// A run of non-special characters: `"hello"`.
    Word(String),
    /// A single space or tab (consecutive spaces collapsed).
    Whitespace,
    /// A single `\n`.
    Newline,
    /// Two or more consecutive newlines (paragraph separator).
    ParagraphBreak,
    /// `\commandname` → stores `"commandname"`.
    Command(String),
    /// `{`
    BeginGroup,
    /// `}`
    EndGroup,
    /// `[`
    BracketOpen,
    /// `]`
    BracketClose,
    /// `$` (single — context determines inline vs display).
    Dollar,
    /// `$$` (display math).
    DollarDollar,
    /// `&` (column separator in tables/align).
    Ampersand,
    /// `\\` (double backslash = line break).
    Backslash,
    /// `~` (non-breaking space).
    Tilde,
    /// `^` (superscript in math).
    Caret,
    /// `_` (subscript in math).
    Underscore,
    /// `#` (parameter token).
    Hash,
    /// `@` (for internal LaTeX2e commands).
    At,
    /// A run of digits with optional decimal point.
    Number(String),
    /// Anything else (punctuation, symbols).
    Special(char),
}

/// Lexer for LaTeX source.
pub struct Lexer<'a> {
    src: &'a str,
    pos: usize,
}

impl<'a> Lexer<'a> {
    /// Create a new lexer for the given source string.
    pub fn new(src: &'a str) -> Self {
        Self { src, pos: 0 }
    }

    /// Tokenize the entire source string.
    pub fn tokenize(&mut self) -> Vec<Token> {
        let mut tokens = Vec::new();

        while self.pos < self.src.len() {
            if let Some(token) = self.next_token() {
                tokens.push(token);
            }
        }

        tokens
    }

    fn next_token(&mut self) -> Option<Token> {
        let ch = self.peek()?;

        match ch {
            // 1. Comment: discard to end of line
            '%' => {
                self.skip_comment();
                None
            }

            // 2-3. Newlines
            '\n' => Some(self.scan_newlines()),

            // 4. Whitespace (space/tab)
            ' ' | '\t' => self.scan_whitespace(),

            // 5. Backslash — commands or \\
            '\\' => {
                let start = self.pos;
                self.advance(); // consume '\'

                match self.peek() {
                    // \\ = Backslash (line break)
                    Some('\\') => {
                        self.advance();
                        Some(Token {
                            kind: TokenKind::Backslash,
                            span: Span::new(start, self.pos),
                        })
                    }
                    // \n or space after backslash = also Backslash
                    Some('\n') | Some(' ') | Some('\t') | None => Some(Token {
                        kind: TokenKind::Backslash,
                        span: Span::new(start, self.pos),
                    }),
                    // Letter → full command name
                    Some(c) if c.is_ascii_alphabetic() => Some(self.scan_command(start)),
                    // Non-letter → single-char command
                    Some(c) => {
                        self.advance();
                        Some(Token {
                            kind: TokenKind::Command(String::from(c)),
                            span: Span::new(start, self.pos),
                        })
                    }
                }
            }

            // 6-7. Dollar signs
            '$' => {
                let start = self.pos;
                self.advance();
                if self.peek() == Some('$') {
                    self.advance();
                    Some(Token {
                        kind: TokenKind::DollarDollar,
                        span: Span::new(start, self.pos),
                    })
                } else {
                    Some(Token {
                        kind: TokenKind::Dollar,
                        span: Span::new(start, self.pos),
                    })
                }
            }

            // 8-9. Braces
            '{' => {
                let start = self.pos;
                self.advance();
                Some(Token {
                    kind: TokenKind::BeginGroup,
                    span: Span::new(start, self.pos),
                })
            }
            '}' => {
                let start = self.pos;
                self.advance();
                Some(Token {
                    kind: TokenKind::EndGroup,
                    span: Span::new(start, self.pos),
                })
            }

            // 10-11. Brackets
            '[' => {
                let start = self.pos;
                self.advance();
                Some(Token {
                    kind: TokenKind::BracketOpen,
                    span: Span::new(start, self.pos),
                })
            }
            ']' => {
                let start = self.pos;
                self.advance();
                Some(Token {
                    kind: TokenKind::BracketClose,
                    span: Span::new(start, self.pos),
                })
            }

            // 12. Ampersand
            '&' => {
                let start = self.pos;
                self.advance();
                Some(Token {
                    kind: TokenKind::Ampersand,
                    span: Span::new(start, self.pos),
                })
            }

            // 13. Tilde
            '~' => {
                let start = self.pos;
                self.advance();
                Some(Token {
                    kind: TokenKind::Tilde,
                    span: Span::new(start, self.pos),
                })
            }

            // 14. Caret
            '^' => {
                let start = self.pos;
                self.advance();
                Some(Token {
                    kind: TokenKind::Caret,
                    span: Span::new(start, self.pos),
                })
            }

            // 15. Underscore
            '_' => {
                let start = self.pos;
                self.advance();
                Some(Token {
                    kind: TokenKind::Underscore,
                    span: Span::new(start, self.pos),
                })
            }

            // 16. Hash
            '#' => {
                let start = self.pos;
                self.advance();
                Some(Token {
                    kind: TokenKind::Hash,
                    span: Span::new(start, self.pos),
                })
            }

            // 17. At
            '@' => {
                let start = self.pos;
                self.advance();
                Some(Token {
                    kind: TokenKind::At,
                    span: Span::new(start, self.pos),
                })
            }

            // 18. Numbers
            '0'..='9' => Some(self.scan_number()),

            // 19. Words (runs of letters)
            c if c.is_ascii_alphabetic() => Some(self.scan_word()),

            // 20. Anything else
            _ => {
                let start = self.pos;
                let c = self.advance()?;
                Some(Token {
                    kind: TokenKind::Special(c),
                    span: Span::new(start, self.pos),
                })
            }
        }
    }

    fn peek(&self) -> Option<char> {
        self.src[self.pos..].chars().next()
    }

    #[allow(dead_code)]
    fn peek2(&self) -> Option<char> {
        let mut chars = self.src[self.pos..].chars();
        chars.next();
        chars.next()
    }

    fn advance(&mut self) -> Option<char> {
        let ch = self.src[self.pos..].chars().next()?;
        self.pos += ch.len_utf8();
        Some(ch)
    }

    fn scan_command(&mut self, start: usize) -> Token {
        // We've already consumed '\' and peeked a letter
        let name_start = self.pos;
        while let Some(c) = self.peek() {
            if c.is_ascii_alphabetic() {
                self.advance();
            } else {
                break;
            }
        }
        let name = self.src[name_start..self.pos].to_string();
        Token {
            kind: TokenKind::Command(name),
            span: Span::new(start, self.pos),
        }
    }

    fn scan_word(&mut self) -> Token {
        let start = self.pos;
        while let Some(c) = self.peek() {
            if c.is_ascii_alphabetic() {
                self.advance();
            } else {
                break;
            }
        }
        let word = self.src[start..self.pos].to_string();
        Token {
            kind: TokenKind::Word(word),
            span: Span::new(start, self.pos),
        }
    }

    fn scan_number(&mut self) -> Token {
        let start = self.pos;
        let mut saw_dot = false;
        while let Some(c) = self.peek() {
            if c.is_ascii_digit() {
                self.advance();
            } else if c == '.' && !saw_dot {
                saw_dot = true;
                self.advance();
            } else {
                break;
            }
        }
        let num = self.src[start..self.pos].to_string();
        Token {
            kind: TokenKind::Number(num),
            span: Span::new(start, self.pos),
        }
    }

    fn skip_comment(&mut self) {
        // Skip '%' and everything until '\n' (but don't consume the newline)
        self.advance(); // consume '%'
        while let Some(c) = self.peek() {
            if c == '\n' {
                break;
            }
            self.advance();
        }
    }

    fn scan_whitespace(&mut self) -> Option<Token> {
        let start = self.pos;
        while let Some(c) = self.peek() {
            if c == ' ' || c == '\t' {
                self.advance();
            } else {
                break;
            }
        }
        Some(Token {
            kind: TokenKind::Whitespace,
            span: Span::new(start, self.pos),
        })
    }

    fn scan_newlines(&mut self) -> Token {
        let start = self.pos;
        let mut newline_count = 0;

        // Count newlines and consume interspersed whitespace
        while let Some(c) = self.peek() {
            match c {
                '\n' => {
                    newline_count += 1;
                    self.advance();
                }
                ' ' | '\t' => {
                    self.advance();
                }
                _ => break,
            }
        }

        if newline_count >= 2 {
            Token {
                kind: TokenKind::ParagraphBreak,
                span: Span::new(start, self.pos),
            }
        } else {
            Token {
                kind: TokenKind::Newline,
                span: Span::new(start, self.pos),
            }
        }
    }
}
