//! Parser for LaTeX token streams.
//!
//! Converts `Vec<Token>` → `ast::Document` using recursive descent.
//! Collects all errors into a `Diagnostics` struct and continues after each error.

use crate::ast;
use crate::error::{AldutexError, Diagnostics, Span};
use crate::lexer::{Token, TokenKind};

/// Recursive-descent parser for LaTeX.
pub struct Parser<'src> {
    tokens: Vec<Token>,
    pos: usize,
    src: &'src str,
    diagnostics: Diagnostics,
    /// Environment stack: tracks open `\begin{...}` environments.
    env_stack: Vec<(String, Span)>,
    /// Footnote accumulator.
    footnotes: Vec<Vec<ast::Inline>>,
}

impl<'src> Parser<'src> {
    /// Create a new parser from a token stream and the original source.
    pub fn new(tokens: Vec<Token>, src: &'src str) -> Self {
        Self {
            tokens,
            pos: 0,
            src,
            diagnostics: Diagnostics::default(),
            env_stack: Vec::new(),
            footnotes: Vec::new(),
        }
    }

    /// Parse the token stream into a `Document` and collected diagnostics.
    pub fn parse(mut self) -> (ast::Document, Diagnostics) {
        let start = 0;
        let preamble = self.parse_preamble();
        let body = self.parse_body();
        let end = self.src.len();

        // Check for unclosed environments
        for (env, span) in &self.env_stack {
            self.diagnostics.push_error(AldutexError::UnmatchedBegin {
                env: env.clone(),
                src: self.src.to_string(),
                span: span.to_source_span(),
            });
        }

        let doc = ast::Document {
            span: Span::new(start, end),
            preamble,
            body,
        };

        (doc, self.diagnostics)
    }

    // ── Token navigation ───────────────────────────────────────

    fn peek(&self) -> Option<&Token> {
        self.tokens.get(self.pos)
    }

    #[allow(dead_code)]
    fn peek2(&self) -> Option<&Token> {
        self.tokens.get(self.pos + 1)
    }

    fn advance(&mut self) -> Option<Token> {
        if self.pos < self.tokens.len() {
            let tok = self.tokens[self.pos].clone();
            self.pos += 1;
            Some(tok)
        } else {
            None
        }
    }

    fn skip_whitespace(&mut self) {
        while let Some(tok) = self.peek() {
            match tok.kind {
                TokenKind::Whitespace | TokenKind::Newline => {
                    self.advance();
                }
                _ => break,
            }
        }
    }

    #[allow(dead_code)]
    fn skip_whitespace_and_newlines(&mut self) {
        while let Some(tok) = self.peek() {
            match tok.kind {
                TokenKind::Whitespace | TokenKind::Newline | TokenKind::ParagraphBreak => {
                    self.advance();
                }
                _ => break,
            }
        }
    }

    fn current_span(&self) -> Span {
        if let Some(tok) = self.peek() {
            tok.span
        } else {
            Span::new(self.src.len(), self.src.len())
        }
    }

    /// Check if current token is a Command with the given name.
    fn is_command(&self, name: &str) -> bool {
        matches!(self.peek(), Some(Token { kind: TokenKind::Command(n), .. }) if n == name)
    }

    /// Read a required brace group as raw text.
    fn read_brace_group_raw(&mut self) -> Option<(String, Span)> {
        let start_span = self.current_span();
        if !matches!(
            self.peek(),
            Some(Token {
                kind: TokenKind::BeginGroup,
                ..
            })
        ) {
            return None;
        }
        self.advance(); // consume {

        let mut depth = 1;
        let content_start = self.current_span().start;
        let mut content_end = content_start;

        while let Some(tok) = self.peek() {
            match tok.kind {
                TokenKind::BeginGroup => {
                    depth += 1;
                    content_end = tok.span.end;
                    self.advance();
                }
                TokenKind::EndGroup => {
                    depth -= 1;
                    if depth == 0 {
                        let end_span = tok.span;
                        self.advance(); // consume }
                        let text = self.src[content_start..content_end].to_string();
                        return Some((text, start_span.merge(end_span)));
                    }
                    content_end = tok.span.end;
                    self.advance();
                }
                _ => {
                    content_end = tok.span.end;
                    self.advance();
                }
            }
        }

        let text = self.src[content_start..content_end].to_string();
        Some((text, start_span.merge(Span::new(content_end, content_end))))
    }

    /// Read an optional bracket group as raw text.
    fn read_bracket_group_raw(&mut self) -> Option<(String, Span)> {
        if !matches!(
            self.peek(),
            Some(Token {
                kind: TokenKind::BracketOpen,
                ..
            })
        ) {
            return None;
        }
        let start_span = self.current_span();
        self.advance(); // consume [

        let mut depth = 1;
        let content_start = self.current_span().start;
        let mut content_end = content_start;

        while let Some(tok) = self.peek() {
            match tok.kind {
                TokenKind::BracketOpen => {
                    depth += 1;
                    content_end = tok.span.end;
                    self.advance();
                }
                TokenKind::BracketClose => {
                    depth -= 1;
                    if depth == 0 {
                        let end_span = tok.span;
                        self.advance(); // consume ]
                        let text = self.src[content_start..content_end].to_string();
                        return Some((text, start_span.merge(end_span)));
                    }
                    content_end = tok.span.end;
                    self.advance();
                }
                _ => {
                    content_end = tok.span.end;
                    self.advance();
                }
            }
        }

        let text = self.src[content_start..content_end].to_string();
        Some((text, start_span.merge(Span::new(content_end, content_end))))
    }

    // ── Preamble parsing ───────────────────────────────────────

    fn parse_preamble(&mut self) -> ast::Preamble {
        let mut document_class = ast::DocumentClass {
            name: "article".to_string(),
            options: Vec::new(),
            span: Span::new(0, 0),
        };
        let mut packages = Vec::new();
        let mut metadata = ast::DocumentMetadata::default();

        // Scan tokens until \begin{document}
        loop {
            self.skip_whitespace_and_newlines();

            if self.pos >= self.tokens.len() {
                break;
            }

            // Check for \begin{document}
            if self.is_command("begin") {
                // Peek ahead to see if it's {document}
                let saved_pos = self.pos;
                self.advance(); // consume \begin
                self.skip_whitespace();
                if let Some((name, _)) = self.read_brace_group_raw() {
                    if name == "document" {
                        break;
                    }
                }
                // Not \begin{document}, restore position
                self.pos = saved_pos;
                self.advance(); // skip this token
                continue;
            }

            if self.is_command("documentclass") {
                let span_start = self.current_span();
                self.advance(); // consume \documentclass
                self.skip_whitespace();

                let options = if let Some((opts, _)) = self.read_bracket_group_raw() {
                    opts.split(',').map(|s| s.trim().to_string()).collect()
                } else {
                    Vec::new()
                };

                self.skip_whitespace();
                if let Some((n, span_end)) = self.read_brace_group_raw() {
                    document_class = ast::DocumentClass {
                        name: n,
                        options,
                        span: span_start.merge(span_end),
                    };
                    continue;
                } else {
                    document_class.options = options;
                    continue;
                }
            }

            if self.is_command("usepackage") {
                let span_start = self.current_span();
                self.advance();
                self.skip_whitespace();

                let options = if let Some((opts, _)) = self.read_bracket_group_raw() {
                    opts.split(',').map(|s| s.trim().to_string()).collect()
                } else {
                    Vec::new()
                };

                self.skip_whitespace();
                if let Some((name, span_end)) = self.read_brace_group_raw() {
                    packages.push(ast::Package {
                        name,
                        options,
                        span: span_start.merge(span_end),
                    });
                }
                continue;
            }

            if self.is_command("title") {
                self.advance();
                self.skip_whitespace();
                metadata.title = Some(self.parse_group_inlines());
                continue;
            }

            if self.is_command("author") {
                self.advance();
                self.skip_whitespace();
                metadata.author = Some(self.parse_group_inlines());
                continue;
            }

            if self.is_command("date") {
                self.advance();
                self.skip_whitespace();
                metadata.date = Some(self.parse_group_inlines());
                continue;
            }

            // Skip any other token in the preamble
            self.advance();
        }

        ast::Preamble {
            document_class,
            packages,
            metadata,
        }
    }

    // ── Body parsing ───────────────────────────────────────────

    fn parse_body(&mut self) -> Vec<ast::Block> {
        let mut blocks = Vec::new();

        loop {
            self.skip_whitespace();

            if self.pos >= self.tokens.len() {
                break;
            }

            // Check for \end{document}
            if self.is_command("end") {
                let saved_pos = self.pos;
                self.advance();
                self.skip_whitespace();
                if let Some((name, _)) = self.read_brace_group_raw() {
                    if name == "document" {
                        break;
                    }
                }
                self.pos = saved_pos;
            }

            // Skip paragraph breaks
            if matches!(
                self.peek(),
                Some(Token {
                    kind: TokenKind::ParagraphBreak,
                    ..
                })
            ) {
                self.advance();
                continue;
            }

            if let Some(block) = self.parse_block() {
                blocks.push(block);
            }
        }

        blocks
    }

    fn parse_block(&mut self) -> Option<ast::Block> {
        let tok = self.peek()?;

        match &tok.kind {
            TokenKind::ParagraphBreak => {
                self.advance();
                None
            }

            TokenKind::Command(name) => {
                let name = name.clone();
                match name.as_str() {
                    "begin" => Some(self.parse_environment()),
                    "section" => Some(self.parse_section(1)),
                    "subsection" => Some(self.parse_section(2)),
                    "subsubsection" => Some(self.parse_section(3)),
                    "paragraph" => Some(self.parse_section(4)),
                    "subparagraph" => Some(self.parse_section(5)),
                    "part" => Some(self.parse_section(0)),
                    "chapter" => Some(self.parse_section(0)),
                    "hline" | "hrule" => {
                        let span = tok.span;
                        self.advance();
                        Some(ast::Block::HRule { span })
                    }
                    "newpage" | "clearpage" => {
                        let span = tok.span;
                        self.advance();
                        Some(ast::Block::PageBreak { span })
                    }
                    "vspace" => Some(self.parse_vspace()),
                    "maketitle" => {
                        // Skip \maketitle for now (it creates title from metadata)
                        self.advance();
                        None
                    }
                    "noindent" | "indent" => {
                        self.advance();
                        None
                    }
                    _ => {
                        // Could be inline command starting a paragraph
                        Some(self.parse_paragraph())
                    }
                }
            }

            _ => Some(self.parse_paragraph()),
        }
    }

    fn parse_section(&mut self, level: u8) -> ast::Block {
        let start = self.current_span();
        self.advance(); // consume \section etc.
        self.skip_whitespace();

        // Optional [short title] — currently ignored
        let _short_title = self.read_bracket_group_raw();

        self.skip_whitespace();

        // Required {title}
        let title = self.parse_group_inlines();

        self.skip_whitespace();

        // Optional \label{key}
        let label = if self.is_command("label") {
            self.advance();
            self.skip_whitespace();
            self.read_brace_group_raw().map(|(s, _)| s)
        } else {
            None
        };

        // Parse body blocks until next section of same or higher level, or end
        let body = self.parse_section_body(level);

        let end = self.current_span();

        ast::Block::Section {
            level,
            title,
            label,
            body,
            span: start.merge(end),
        }
    }

    fn parse_section_body(&mut self, level: u8) -> Vec<ast::Block> {
        let mut blocks = Vec::new();

        loop {
            self.skip_whitespace();

            if self.pos >= self.tokens.len() {
                break;
            }

            // Check for \end{document}
            if self.is_command("end") {
                break;
            }

            // Check if next section is same or higher level
            if let Some(Token {
                kind: TokenKind::Command(name),
                ..
            }) = self.peek()
            {
                let next_level = match name.as_str() {
                    "part" | "chapter" => Some(0u8),
                    "section" => Some(1),
                    "subsection" => Some(2),
                    "subsubsection" => Some(3),
                    "paragraph" => Some(4),
                    "subparagraph" => Some(5),
                    _ => None,
                };

                if let Some(nl) = next_level {
                    if nl <= level {
                        break; // Same or higher level section ends this one
                    }
                }
            }

            // Skip paragraph breaks
            if matches!(
                self.peek(),
                Some(Token {
                    kind: TokenKind::ParagraphBreak,
                    ..
                })
            ) {
                self.advance();
                continue;
            }

            if let Some(block) = self.parse_block() {
                blocks.push(block);
            }
        }

        blocks
    }

    fn parse_vspace(&mut self) -> ast::Block {
        let start = self.current_span();
        self.advance(); // consume \vspace
        self.skip_whitespace();

        // Optional *
        // Skip it for now
        let amount_pt = if let Some((raw, _)) = self.read_brace_group_raw() {
            parse_length_to_pt(&raw)
        } else {
            0.0
        };

        ast::Block::VSpace {
            amount_pt,
            span: start.merge(self.current_span()),
        }
    }

    fn parse_environment(&mut self) -> ast::Block {
        let start = self.current_span();
        self.advance(); // consume \begin
        self.skip_whitespace();

        let env_name = if let Some((name, _)) = self.read_brace_group_raw() {
            name
        } else {
            return ast::Block::RawCommand {
                name: "begin".to_string(),
                args: Vec::new(),
                span: start.merge(self.current_span()),
            };
        };

        self.env_stack.push((env_name.clone(), start));

        let block = match env_name.as_str() {
            "itemize" => self.parse_list(ast::ListKind::Itemize, &env_name, start),
            "enumerate" => self.parse_list(ast::ListKind::Enumerate, &env_name, start),
            "description" => self.parse_list(ast::ListKind::Description, &env_name, start),
            "figure" | "figure*" => self.parse_figure(&env_name, start),
            "table" | "table*" => self.parse_table_env(&env_name, start),
            "tabular" => self.parse_tabular(&env_name, start),
            "equation" | "equation*" | "displaymath" => {
                self.parse_display_math_env(&env_name, start)
            }
            "align" | "align*" => self.parse_display_math_env(&env_name, start),
            "gather" | "gather*" => self.parse_display_math_env(&env_name, start),
            "verbatim" | "lstlisting" => self.parse_verbatim(&env_name, start),
            "abstract" => self.parse_abstract_env(&env_name, start),
            "document" => {
                // Nested \begin{document} is an error
                self.diagnostics.push_error(AldutexError::UnexpectedToken {
                    token: "\\begin{document}".to_string(),
                    expected: "content".to_string(),
                    src: self.src.to_string(),
                    span: start.to_source_span(),
                });
                self.skip_to_end_env(&env_name);
                ast::Block::RawCommand {
                    name: "begin".to_string(),
                    args: Vec::new(),
                    span: start.merge(self.current_span()),
                }
            }
            _ => {
                // Unknown environment
                self.diagnostics.push_warning(AldutexError::UnknownCommand {
                    name: format!("begin{{{env_name}}}"),
                    src: self.src.to_string(),
                    span: start.to_source_span(),
                });
                self.skip_to_end_env(&env_name);
                ast::Block::RawCommand {
                    name: format!("begin{{{env_name}}}"),
                    args: Vec::new(),
                    span: start.merge(self.current_span()),
                }
            }
        };

        // Pop environment from stack
        if let Some(idx) = self.env_stack.iter().rposition(|(n, _)| n == &env_name) {
            self.env_stack.remove(idx);
        }

        block
    }

    fn skip_to_end_env(&mut self, env_name: &str) {
        let mut depth = 1;
        while let Some(tok) = self.advance() {
            if let TokenKind::Command(ref name) = tok.kind {
                if name == "begin" {
                    self.skip_whitespace();
                    if let Some((n, _)) = self.read_brace_group_raw() {
                        if n == env_name {
                            depth += 1;
                        }
                    }
                } else if name == "end" {
                    self.skip_whitespace();
                    if let Some((n, _)) = self.read_brace_group_raw() {
                        if n == env_name {
                            depth -= 1;
                            if depth == 0 {
                                return;
                            }
                        }
                    }
                }
            }
        }
    }

    fn consume_end_env(&mut self, env_name: &str) {
        self.skip_whitespace();
        // Skip newlines too
        while matches!(
            self.peek(),
            Some(Token {
                kind: TokenKind::Newline | TokenKind::ParagraphBreak,
                ..
            })
        ) {
            self.advance();
        }

        if self.is_command("end") {
            self.advance();
            self.skip_whitespace();
            if let Some((name, _)) = self.read_brace_group_raw() {
                if name != env_name {
                    self.diagnostics.push_error(AldutexError::UnmatchedEnd {
                        env: name,
                        src: self.src.to_string(),
                        span: self.current_span().to_source_span(),
                    });
                }
            }
        }
    }

    fn parse_list(&mut self, kind: ast::ListKind, env_name: &str, start: Span) -> ast::Block {
        let mut items = Vec::new();

        loop {
            self.skip_whitespace();
            // Also skip newlines and paragraph breaks
            while matches!(
                self.peek(),
                Some(Token {
                    kind: TokenKind::Newline | TokenKind::ParagraphBreak,
                    ..
                })
            ) {
                self.advance();
                self.skip_whitespace();
            }

            if self.pos >= self.tokens.len() {
                break;
            }

            // Check for \end{env_name}
            if self.is_command("end") {
                break;
            }

            if self.is_command("item") {
                items.push(self.parse_list_item(env_name));
            } else {
                // Skip unexpected tokens
                self.advance();
            }
        }

        self.consume_end_env(env_name);

        ast::Block::List {
            kind,
            items,
            span: start.merge(self.current_span()),
        }
    }

    fn parse_list_item(&mut self, _parent_env_name: &str) -> ast::ListItem {
        let start = self.current_span();
        self.advance(); // consume \item
        self.skip_whitespace();

        // Optional label: \item[label]
        let label = if matches!(
            self.peek(),
            Some(Token {
                kind: TokenKind::BracketOpen,
                ..
            })
        ) {
            let saved = self.pos;
            if let Some((text, _)) = self.read_bracket_group_raw() {
                Some(vec![ast::Inline::Text {
                    content: text,
                    span: self.current_span(),
                }])
            } else {
                self.pos = saved;
                None
            }
        } else {
            None
        };

        // Parse content blocks until next \item or \end{env}. Do NOT skip
        // whitespace inside the loop — parse_inline emits whitespace as a
        // " " text inline, which is needed to keep words apart. We only trim
        // leading/trailing whitespace when flushing each paragraph.
        let mut content = Vec::new();
        let mut current_inlines = Vec::new();

        let flush =
            |inlines: &mut Vec<ast::Inline>, content: &mut Vec<ast::Block>| {
                trim_inlines(inlines);
                if !inlines.is_empty() {
                    let span = inlines_span(inlines);
                    content.push(ast::Block::Paragraph {
                        inlines: std::mem::take(inlines),
                        span,
                    });
                }
            };

        loop {
            if self.pos >= self.tokens.len() {
                break;
            }

            // Check for \end{env}
            if self.is_command("end") {
                break;
            }

            // Check for next \item
            if self.is_command("item") {
                break;
            }

            // Check for nested \begin{...}
            if self.is_command("begin") {
                flush(&mut current_inlines, &mut content);
                content.push(self.parse_environment());
                continue;
            }

            // Paragraph break — flush current inlines and start a new paragraph.
            if matches!(
                self.peek(),
                Some(Token {
                    kind: TokenKind::ParagraphBreak,
                    ..
                })
            ) {
                flush(&mut current_inlines, &mut content);
                self.advance();
                continue;
            }

            if let Some(inline) = self.parse_inline() {
                current_inlines.push(inline);
            } else {
                break;
            }
        }

        flush(&mut current_inlines, &mut content);

        ast::ListItem {
            label,
            content,
            span: start.merge(self.current_span()),
        }
    }

    fn parse_figure(&mut self, env_name: &str, start: Span) -> ast::Block {
        // Optional placement: [htbp]
        self.skip_whitespace();
        let placement = if let Some((p, _)) = self.read_bracket_group_raw() {
            p
        } else {
            "htbp".to_string()
        };

        let mut content = Vec::new();
        let mut caption = None;
        let mut label = None;

        loop {
            self.skip_whitespace();
            while matches!(
                self.peek(),
                Some(Token {
                    kind: TokenKind::Newline | TokenKind::ParagraphBreak,
                    ..
                })
            ) {
                self.advance();
                self.skip_whitespace();
            }

            if self.pos >= self.tokens.len() {
                break;
            }

            if self.is_command("end") {
                break;
            }

            if self.is_command("caption") {
                self.advance();
                self.skip_whitespace();
                caption = Some(self.parse_group_inlines());
                continue;
            }

            if self.is_command("label") {
                self.advance();
                self.skip_whitespace();
                label = self.read_brace_group_raw().map(|(s, _)| s);
                continue;
            }

            if self.is_command("centering") {
                self.advance();
                continue;
            }

            if let Some(block) = self.parse_block() {
                content.push(block);
            }
        }

        self.consume_end_env(env_name);

        ast::Block::Figure {
            content,
            caption,
            label,
            placement,
            span: start.merge(self.current_span()),
        }
    }

    fn parse_table_env(&mut self, env_name: &str, start: Span) -> ast::Block {
        // A table environment wraps a tabular
        self.skip_whitespace();

        // Optional placement
        let _placement = self.read_bracket_group_raw();

        let mut _inner_blocks: Vec<ast::Block> = Vec::new();
        let mut caption = None;
        let mut label = None;
        let mut spec = String::new();
        let mut rows = Vec::new();

        loop {
            self.skip_whitespace();
            while matches!(
                self.peek(),
                Some(Token {
                    kind: TokenKind::Newline | TokenKind::ParagraphBreak,
                    ..
                })
            ) {
                self.advance();
                self.skip_whitespace();
            }

            if self.pos >= self.tokens.len() {
                break;
            }

            if self.is_command("end") {
                let saved = self.pos;
                self.advance();
                self.skip_whitespace();
                if let Some((name, _)) = self.read_brace_group_raw() {
                    if name == env_name {
                        break;
                    }
                }
                self.pos = saved;
            }

            if self.is_command("caption") {
                self.advance();
                self.skip_whitespace();
                caption = Some(self.parse_group_inlines());
                continue;
            }

            if self.is_command("label") {
                self.advance();
                self.skip_whitespace();
                label = self.read_brace_group_raw().map(|(s, _)| s);
                continue;
            }

            if self.is_command("centering") {
                self.advance();
                continue;
            }

            if self.is_command("begin") {
                let saved = self.pos;
                self.advance();
                self.skip_whitespace();
                if let Some((name, _)) = self.read_brace_group_raw() {
                    if name == "tabular" {
                        self.env_stack
                            .push(("tabular".to_string(), self.current_span()));
                        self.skip_whitespace();
                        if let Some((s, _)) = self.read_brace_group_raw() {
                            spec = s;
                        }
                        rows = self.parse_tabular_rows("tabular");
                        self.consume_end_env("tabular");
                        if let Some(idx) = self.env_stack.iter().rposition(|(n, _)| n == "tabular")
                        {
                            self.env_stack.remove(idx);
                        }
                        continue;
                    }
                }
                self.pos = saved;
            }

            // Skip other tokens
            self.advance();
        }

        ast::Block::Table {
            spec,
            rows,
            caption,
            label,
            span: start.merge(self.current_span()),
        }
    }

    fn parse_tabular(&mut self, env_name: &str, start: Span) -> ast::Block {
        self.skip_whitespace();

        // Read column spec
        let spec = if let Some((s, _)) = self.read_brace_group_raw() {
            s
        } else {
            String::new()
        };

        let rows = self.parse_tabular_rows(env_name);
        self.consume_end_env(env_name);

        ast::Block::Table {
            spec,
            rows,
            caption: None,
            label: None,
            span: start.merge(self.current_span()),
        }
    }

    fn parse_tabular_rows(&mut self, _env_name: &str) -> Vec<ast::TableRow> {
        let mut rows = Vec::new();
        let mut current_cells: Vec<ast::TableCell> = Vec::new();
        let mut current_inlines: Vec<ast::Inline> = Vec::new();
        let mut row_start = self.current_span();

        loop {
            self.skip_whitespace();

            if self.pos >= self.tokens.len() {
                break;
            }

            // Check for \end
            if self.is_command("end") {
                break;
            }

            // \hline — skip (treated as decoration, not data)
            if self.is_command("hline") {
                self.advance();
                continue;
            }

            // & — cell separator
            if matches!(
                self.peek(),
                Some(Token {
                    kind: TokenKind::Ampersand,
                    ..
                })
            ) {
                let span = inlines_span_or(&current_inlines, self.current_span());
                current_cells.push(ast::TableCell {
                    content: std::mem::take(&mut current_inlines),
                    colspan: 1,
                    span,
                });
                self.advance();
                continue;
            }

            // \\ — row separator
            if matches!(
                self.peek(),
                Some(Token {
                    kind: TokenKind::Backslash,
                    ..
                })
            ) {
                let span = inlines_span_or(&current_inlines, self.current_span());
                current_cells.push(ast::TableCell {
                    content: std::mem::take(&mut current_inlines),
                    colspan: 1,
                    span,
                });
                let row_end = self.current_span();
                rows.push(ast::TableRow {
                    cells: std::mem::take(&mut current_cells),
                    span: row_start.merge(row_end),
                });
                self.advance();
                row_start = self.current_span();
                continue;
            }

            // Skip newlines
            if matches!(
                self.peek(),
                Some(Token {
                    kind: TokenKind::Newline | TokenKind::ParagraphBreak,
                    ..
                })
            ) {
                self.advance();
                continue;
            }

            // Parse inline content for current cell
            if let Some(inline) = self.parse_inline() {
                current_inlines.push(inline);
            } else {
                break;
            }
        }

        // Flush remaining cell/row
        if !current_inlines.is_empty() || !current_cells.is_empty() {
            let span = inlines_span_or(&current_inlines, self.current_span());
            current_cells.push(ast::TableCell {
                content: current_inlines,
                colspan: 1,
                span,
            });
            let row_end = self.current_span();
            rows.push(ast::TableRow {
                cells: current_cells,
                span: row_start.merge(row_end),
            });
        }

        rows
    }

    fn parse_display_math_env(&mut self, env_name: &str, start: Span) -> ast::Block {
        // Parse math content until \end{env_name}
        let node = self.parse_math_until_end_env(env_name);
        self.consume_end_env(env_name);

        ast::Block::MathBlock {
            node,
            span: start.merge(self.current_span()),
        }
    }

    fn parse_verbatim(&mut self, env_name: &str, start: Span) -> ast::Block {
        // Read raw text until \end{verbatim}
        let content_start = self.current_span().start;
        let end_marker = format!("\\end{{{env_name}}}");

        // Find \end{verbatim} in the raw source
        if let Some(end_pos) = self.src[content_start..].find(&end_marker) {
            let content = self.src[content_start..content_start + end_pos].to_string();

            // Advance tokens past the content
            let target_pos = content_start + end_pos + end_marker.len();
            while self.pos < self.tokens.len() && self.tokens[self.pos].span.end <= target_pos {
                self.advance();
            }

            ast::Block::Verbatim {
                content,
                span: start.merge(Span::new(target_pos, target_pos)),
            }
        } else {
            // No matching \end found
            let content = self.src[content_start..].to_string();
            // Consume all remaining tokens
            while self.pos < self.tokens.len() {
                self.advance();
            }
            ast::Block::Verbatim {
                content,
                span: start.merge(Span::new(self.src.len(), self.src.len())),
            }
        }
    }

    fn parse_abstract_env(&mut self, env_name: &str, start: Span) -> ast::Block {
        let mut blocks = Vec::new();

        loop {
            self.skip_whitespace();
            while matches!(
                self.peek(),
                Some(Token {
                    kind: TokenKind::Newline | TokenKind::ParagraphBreak,
                    ..
                })
            ) {
                self.advance();
                self.skip_whitespace();
            }

            if self.pos >= self.tokens.len() {
                break;
            }

            if self.is_command("end") {
                break;
            }

            if let Some(block) = self.parse_block() {
                blocks.push(block);
            }
        }

        self.consume_end_env(env_name);

        // Return as a paragraph block for now
        ast::Block::RawCommand {
            name: "abstract".to_string(),
            args: Vec::new(),
            span: start.merge(self.current_span()),
        }
    }

    // ── Paragraph & inline parsing ─────────────────────────────

    fn parse_paragraph(&mut self) -> ast::Block {
        let start = self.current_span();
        let mut inlines = Vec::new();

        loop {
            if self.pos >= self.tokens.len() {
                break;
            }

            // End paragraph on ParagraphBreak
            if matches!(
                self.peek(),
                Some(Token {
                    kind: TokenKind::ParagraphBreak,
                    ..
                })
            ) {
                break;
            }

            // End paragraph on block-level commands
            if let Some(Token {
                kind: TokenKind::Command(name),
                ..
            }) = self.peek()
            {
                match name.as_str() {
                    "begin" | "end" | "section" | "subsection" | "subsubsection" | "paragraph"
                    | "subparagraph" | "part" | "chapter" | "hline" | "hrule" | "newpage"
                    | "clearpage" | "vspace" | "maketitle" => break,
                    _ => {}
                }
            }

            if let Some(inline) = self.parse_inline() {
                inlines.push(inline);
            } else {
                break;
            }
        }

        // Trim leading/trailing whitespace
        trim_inlines(&mut inlines);

        let span = if inlines.is_empty() {
            start
        } else {
            start.merge(inlines_span(&inlines))
        };

        ast::Block::Paragraph { inlines, span }
    }

    fn parse_inline(&mut self) -> Option<ast::Inline> {
        let tok = self.peek()?;

        match &tok.kind {
            TokenKind::Word(w) => {
                let inline = ast::Inline::Text {
                    content: w.clone(),
                    span: tok.span,
                };
                self.advance();
                Some(inline)
            }

            TokenKind::Whitespace | TokenKind::Newline => {
                let span = tok.span;
                self.advance();
                Some(ast::Inline::Text {
                    content: " ".to_string(),
                    span,
                })
            }

            TokenKind::Tilde => {
                let span = tok.span;
                self.advance();
                Some(ast::Inline::NonBreakingSpace { span })
            }

            TokenKind::Dollar => Some(self.parse_inline_math()),

            TokenKind::DollarDollar => {
                // Display math inside paragraph — could be intentional
                // Parse as inline math block
                let start = tok.span;
                self.advance(); // consume $$
                let node = self.parse_math_until_dollar_dollar();
                let end = self.current_span();
                Some(ast::Inline::Math {
                    node,
                    span: start.merge(end),
                })
            }

            TokenKind::Backslash => {
                let span = tok.span;
                self.advance();
                Some(ast::Inline::LineBreak { span })
            }

            TokenKind::Command(name) => {
                let name = name.clone();
                match name.as_str() {
                    "textbf" | "bf" => {
                        let start = tok.span;
                        self.advance();
                        self.skip_whitespace();
                        let content = self.parse_group_inlines();
                        let end = self.current_span();
                        Some(ast::Inline::Bold {
                            content,
                            span: start.merge(end),
                        })
                    }
                    "textit" | "it" => {
                        let start = tok.span;
                        self.advance();
                        self.skip_whitespace();
                        let content = self.parse_group_inlines();
                        let end = self.current_span();
                        Some(ast::Inline::Italic {
                            content,
                            span: start.merge(end),
                        })
                    }
                    "texttt" | "tt" => {
                        let start = tok.span;
                        self.advance();
                        self.skip_whitespace();
                        let content = self.parse_group_inlines();
                        let end = self.current_span();
                        Some(ast::Inline::Monospace {
                            content,
                            span: start.merge(end),
                        })
                    }
                    "textsc" => {
                        let start = tok.span;
                        self.advance();
                        self.skip_whitespace();
                        let content = self.parse_group_inlines();
                        let end = self.current_span();
                        Some(ast::Inline::SmallCaps {
                            content,
                            span: start.merge(end),
                        })
                    }
                    "emph" => {
                        let start = tok.span;
                        self.advance();
                        self.skip_whitespace();
                        let content = self.parse_group_inlines();
                        let end = self.current_span();
                        Some(ast::Inline::Emph {
                            content,
                            span: start.merge(end),
                        })
                    }
                    "underline" | "uline" => {
                        let start = tok.span;
                        self.advance();
                        self.skip_whitespace();
                        let content = self.parse_group_inlines();
                        let end = self.current_span();
                        Some(ast::Inline::Underline {
                            content,
                            span: start.merge(end),
                        })
                    }
                    "textrm" | "textsf" | "textup" | "textsl" | "textmd" | "textlf" => {
                        // These just pass through content for now
                        let start = tok.span;
                        self.advance();
                        self.skip_whitespace();
                        let content = self.parse_group_inlines();
                        let end = self.current_span();
                        // Return as regular text wrapper
                        if content.len() == 1 {
                            Some(content.into_iter().next().unwrap_or(ast::Inline::Text {
                                content: String::new(),
                                span: start.merge(end),
                            }))
                        } else {
                            Some(ast::Inline::Text {
                                content: inlines_to_string(&content),
                                span: start.merge(end),
                            })
                        }
                    }
                    "href" => {
                        let start = tok.span;
                        self.advance();
                        self.skip_whitespace();
                        let url = self
                            .read_brace_group_raw()
                            .map(|(s, _)| s)
                            .unwrap_or_default();
                        self.skip_whitespace();
                        let text = self.parse_group_inlines();
                        let end = self.current_span();
                        Some(ast::Inline::Link {
                            url,
                            text,
                            span: start.merge(end),
                        })
                    }
                    "url" => {
                        let start = tok.span;
                        self.advance();
                        self.skip_whitespace();
                        let url = self
                            .read_brace_group_raw()
                            .map(|(s, _)| s)
                            .unwrap_or_default();
                        let end = self.current_span();
                        Some(ast::Inline::Link {
                            url: url.clone(),
                            text: vec![ast::Inline::Monospace {
                                content: vec![ast::Inline::Text {
                                    content: url,
                                    span: start.merge(end),
                                }],
                                span: start.merge(end),
                            }],
                            span: start.merge(end),
                        })
                    }
                    "ref" | "pageref" => {
                        let start = tok.span;
                        self.advance();
                        self.skip_whitespace();
                        let label = self
                            .read_brace_group_raw()
                            .map(|(s, _)| s)
                            .unwrap_or_default();
                        let end = self.current_span();
                        Some(ast::Inline::Ref {
                            label,
                            span: start.merge(end),
                        })
                    }
                    "cite" => {
                        let start = tok.span;
                        self.advance();
                        self.skip_whitespace();
                        // Optional [note]
                        let _note = self.read_bracket_group_raw();
                        self.skip_whitespace();
                        let keys_raw = self
                            .read_brace_group_raw()
                            .map(|(s, _)| s)
                            .unwrap_or_default();
                        let keys: Vec<String> = keys_raw
                            .split(',')
                            .map(|s| s.trim().to_string())
                            .filter(|s| !s.is_empty())
                            .collect();
                        let end = self.current_span();
                        Some(ast::Inline::Citation {
                            keys,
                            span: start.merge(end),
                        })
                    }
                    "footnote" => {
                        let start = tok.span;
                        self.advance();
                        self.skip_whitespace();
                        let content = self.parse_group_inlines();
                        let index = self.footnotes.len();
                        self.footnotes.push(content);
                        let end = self.current_span();
                        Some(ast::Inline::FootnoteRef {
                            index,
                            span: start.merge(end),
                        })
                    }
                    "label" => {
                        // Consume and skip — labels attach to parent
                        self.advance();
                        self.skip_whitespace();
                        let _ = self.read_brace_group_raw();
                        // Return None so it doesn't appear in inline content
                        // Try to get next inline instead
                        self.parse_inline()
                    }
                    "hspace" => {
                        let start = tok.span;
                        self.advance();
                        self.skip_whitespace();
                        let amount_pt = if let Some((raw, _)) = self.read_brace_group_raw() {
                            parse_length_to_pt(&raw)
                        } else {
                            0.0
                        };
                        let end = self.current_span();
                        Some(ast::Inline::HSpace {
                            amount_pt,
                            span: start.merge(end),
                        })
                    }
                    "quad" => {
                        let span = tok.span;
                        self.advance();
                        Some(ast::Inline::HSpace {
                            amount_pt: 10.0, // approx 1em at 10pt
                            span,
                        })
                    }
                    "qquad" => {
                        let span = tok.span;
                        self.advance();
                        Some(ast::Inline::HSpace {
                            amount_pt: 20.0,
                            span,
                        })
                    }
                    "ldots" | "dots" => {
                        let span = tok.span;
                        self.advance();
                        Some(ast::Inline::Text {
                            content: "\u{2026}".to_string(),
                            span,
                        })
                    }
                    "LaTeX" => {
                        let span = tok.span;
                        self.advance();
                        Some(ast::Inline::Text {
                            content: "LaTeX".to_string(),
                            span,
                        })
                    }
                    "TeX" => {
                        let span = tok.span;
                        self.advance();
                        Some(ast::Inline::Text {
                            content: "TeX".to_string(),
                            span,
                        })
                    }
                    "today" => {
                        let span = tok.span;
                        self.advance();
                        let now = "April 25, 2026"; // TODO: use actual current date
                        Some(ast::Inline::Text {
                            content: now.to_string(),
                            span,
                        })
                    }
                    // Escaped special characters
                    "%" => {
                        let span = tok.span;
                        self.advance();
                        Some(ast::Inline::Text {
                            content: "%".to_string(),
                            span,
                        })
                    }
                    "$" => {
                        let span = tok.span;
                        self.advance();
                        Some(ast::Inline::Text {
                            content: "$".to_string(),
                            span,
                        })
                    }
                    "&" => {
                        let span = tok.span;
                        self.advance();
                        Some(ast::Inline::Text {
                            content: "&".to_string(),
                            span,
                        })
                    }
                    "#" => {
                        let span = tok.span;
                        self.advance();
                        Some(ast::Inline::Text {
                            content: "#".to_string(),
                            span,
                        })
                    }
                    "_" => {
                        let span = tok.span;
                        self.advance();
                        Some(ast::Inline::Text {
                            content: "_".to_string(),
                            span,
                        })
                    }
                    "{" => {
                        let span = tok.span;
                        self.advance();
                        Some(ast::Inline::Text {
                            content: "{".to_string(),
                            span,
                        })
                    }
                    "}" => {
                        let span = tok.span;
                        self.advance();
                        Some(ast::Inline::Text {
                            content: "}".to_string(),
                            span,
                        })
                    }
                    // Thin spaces and spacing commands
                    "," => {
                        let span = tok.span;
                        self.advance();
                        Some(ast::Inline::HSpace {
                            amount_pt: 1.67, // thin space
                            span,
                        })
                    }
                    ";" => {
                        let span = tok.span;
                        self.advance();
                        Some(ast::Inline::HSpace {
                            amount_pt: 2.77, // medium space
                            span,
                        })
                    }
                    ":" => {
                        let span = tok.span;
                        self.advance();
                        Some(ast::Inline::HSpace {
                            amount_pt: 2.22, // thick space
                            span,
                        })
                    }
                    "!" => {
                        let span = tok.span;
                        self.advance();
                        Some(ast::Inline::HSpace {
                            amount_pt: -1.67, // negative thin space
                            span,
                        })
                    }
                    // Font size commands — skip (affect layout but not AST structure)
                    "tiny" | "scriptsize" | "footnotesize" | "small" | "normalsize" | "large"
                    | "Large" | "LARGE" | "huge" | "Huge" => {
                        self.advance();
                        self.parse_inline()
                    }
                    // Skip these layout hints
                    "smallskip" | "medskip" | "bigskip" | "noindent" | "indent" => {
                        self.advance();
                        self.parse_inline()
                    }
                    // \includegraphics
                    "includegraphics" => {
                        let start = tok.span;
                        self.advance();
                        self.skip_whitespace();
                        let _opts = self.read_bracket_group_raw();
                        self.skip_whitespace();
                        let path = self
                            .read_brace_group_raw()
                            .map(|(s, _)| s)
                            .unwrap_or_default();
                        let end = self.current_span();
                        Some(ast::Inline::RawInlineCmd {
                            name: "includegraphics".to_string(),
                            args: vec![ast::Arg {
                                kind: ast::ArgKind::Required,
                                content: ast::ArgContent::Raw(path),
                                span: start.merge(end),
                            }],
                            span: start.merge(end),
                        })
                    }
                    // Accent commands
                    "'" | "`" | "^" | "\"" | "~" | "=" | "." | "c" | "v" | "u" | "H" | "d"
                    | "b" | "k" | "r" | "t" => {
                        let start = tok.span;
                        let accent = name.clone();
                        self.advance();
                        self.skip_whitespace();
                        let base = if let Some((ch, _)) = self.read_brace_group_raw() {
                            ch
                        } else if let Some(next) = self.peek() {
                            if let TokenKind::Word(w) = &next.kind {
                                let first = w.chars().next().unwrap_or(' ');
                                self.advance();
                                first.to_string()
                            } else {
                                String::new()
                            }
                        } else {
                            String::new()
                        };
                        let end = self.current_span();
                        let result = apply_accent(&accent, &base);
                        Some(ast::Inline::Text {
                            content: result,
                            span: start.merge(end),
                        })
                    }
                    // Dashes are actually handled by the lexer as special chars,
                    // but if someone uses \textendash or \textemdash:
                    "textendash" => {
                        let span = tok.span;
                        self.advance();
                        Some(ast::Inline::Text {
                            content: "\u{2013}".to_string(),
                            span,
                        })
                    }
                    "textemdash" => {
                        let span = tok.span;
                        self.advance();
                        Some(ast::Inline::Text {
                            content: "\u{2014}".to_string(),
                            span,
                        })
                    }
                    "cdot" => {
                        let span = tok.span;
                        self.advance();
                        Some(ast::Inline::Text {
                            content: "\u{00B7}".to_string(),
                            span,
                        })
                    }
                    // \sout (strikethrough) — treat as underline variant for now
                    "sout" => {
                        let start = tok.span;
                        self.advance();
                        self.skip_whitespace();
                        let _content = self.parse_group_inlines();
                        let end = self.current_span();
                        Some(ast::Inline::RawInlineCmd {
                            name: "sout".to_string(),
                            args: Vec::new(),
                            span: start.merge(end),
                        })
                    }
                    "MakeUppercase" => {
                        let start = tok.span;
                        self.advance();
                        self.skip_whitespace();
                        let content = self.parse_group_inlines();
                        let end = self.current_span();
                        // Convert to uppercase text
                        let text = inlines_to_string(&content).to_uppercase();
                        Some(ast::Inline::Text {
                            content: text,
                            span: start.merge(end),
                        })
                    }
                    "MakeLowercase" => {
                        let start = tok.span;
                        self.advance();
                        self.skip_whitespace();
                        let content = self.parse_group_inlines();
                        let end = self.current_span();
                        let text = inlines_to_string(&content).to_lowercase();
                        Some(ast::Inline::Text {
                            content: text,
                            span: start.merge(end),
                        })
                    }
                    "[" => {
                        let start = tok.span;
                        self.advance(); // consume \[
                        let node = self.parse_math_until_display_bracket();
                        let end = self.current_span();
                        Some(ast::Inline::Math {
                            node,
                            span: start.merge(end),
                        })
                    }
                    // Unknown command
                    _ => {
                        let start = tok.span;
                        let cmd_name = name.clone();
                        self.advance();

                        // Try to consume arguments
                        let mut args = Vec::new();
                        self.skip_whitespace();

                        // Optional args
                        while let Some((raw, span)) = self.read_bracket_group_raw() {
                            args.push(ast::Arg {
                                kind: ast::ArgKind::Optional,
                                content: ast::ArgContent::Raw(raw),
                                span,
                            });
                            self.skip_whitespace();
                        }

                        // Required args
                        while let Some((raw, span)) = self.read_brace_group_raw() {
                            args.push(ast::Arg {
                                kind: ast::ArgKind::Required,
                                content: ast::ArgContent::Raw(raw),
                                span,
                            });
                            self.skip_whitespace();
                        }

                        let end = self.current_span();

                        self.diagnostics.push_warning(AldutexError::UnknownCommand {
                            name: cmd_name.clone(),
                            src: self.src.to_string(),
                            span: start.to_source_span(),
                        });

                        Some(ast::Inline::RawInlineCmd {
                            name: cmd_name,
                            args,
                            span: start.merge(end),
                        })
                    }
                }
            }

            TokenKind::Number(n) => {
                let inline = ast::Inline::Text {
                    content: n.clone(),
                    span: tok.span,
                };
                self.advance();
                Some(inline)
            }

            TokenKind::Special(c) => {
                let ch = *c;
                let span = tok.span;
                self.advance();

                // Handle em-dash and en-dash from consecutive hyphens
                if ch == '-' {
                    let mut dashes = 1;
                    while matches!(
                        self.peek(),
                        Some(Token {
                            kind: TokenKind::Special('-'),
                            ..
                        })
                    ) {
                        dashes += 1;
                        self.advance();
                    }
                    let content = match dashes {
                        1 => "-".to_string(),
                        2 => "\u{2013}".to_string(), // en-dash
                        _ => "\u{2014}".to_string(), // em-dash (3+)
                    };
                    return Some(ast::Inline::Text {
                        content,
                        span: span.merge(self.current_span()),
                    });
                }

                Some(ast::Inline::Text {
                    content: ch.to_string(),
                    span,
                })
            }

            TokenKind::BeginGroup => {
                // Bare group {content} — parse as inline group
                let group_span = tok.span;
                self.advance();
                let mut content = Vec::new();
                loop {
                    if self.pos >= self.tokens.len() {
                        break;
                    }
                    if matches!(
                        self.peek(),
                        Some(Token {
                            kind: TokenKind::EndGroup,
                            ..
                        })
                    ) {
                        self.advance();
                        break;
                    }
                    if let Some(inline) = self.parse_inline() {
                        content.push(inline);
                    } else {
                        break;
                    }
                }
                if content.len() == 1 {
                    content.into_iter().next()
                } else if content.is_empty() {
                    self.parse_inline()
                } else {
                    // Return as first inline (flatten group)
                    Some(content.into_iter().next().unwrap_or(ast::Inline::Text {
                        content: String::new(),
                        span: group_span,
                    }))
                }
            }

            TokenKind::EndGroup => {
                // Unmatched } — return None to end parsing this group
                None
            }

            TokenKind::ParagraphBreak => None,

            TokenKind::Caret | TokenKind::Underscore | TokenKind::Hash | TokenKind::Ampersand => {
                // These are special in math mode but can appear in text
                let span = tok.span;
                let content = match &tok.kind {
                    TokenKind::Caret => "^",
                    TokenKind::Underscore => "_",
                    TokenKind::Hash => "#",
                    TokenKind::Ampersand => "&",
                    _ => "",
                };
                self.advance();
                Some(ast::Inline::Text {
                    content: content.to_string(),
                    span,
                })
            }

            TokenKind::At => {
                let span = tok.span;
                self.advance();
                Some(ast::Inline::Text {
                    content: "@".to_string(),
                    span,
                })
            }

            _ => {
                // Skip unexpected tokens
                self.advance();
                self.parse_inline()
            }
        }
    }

    fn parse_group_inlines(&mut self) -> Vec<ast::Inline> {
        if !matches!(
            self.peek(),
            Some(Token {
                kind: TokenKind::BeginGroup,
                ..
            })
        ) {
            return Vec::new();
        }
        self.advance(); // consume {

        let mut inlines = Vec::new();

        loop {
            if self.pos >= self.tokens.len() {
                break;
            }

            if matches!(
                self.peek(),
                Some(Token {
                    kind: TokenKind::EndGroup,
                    ..
                })
            ) {
                self.advance(); // consume }
                break;
            }

            if let Some(inline) = self.parse_inline() {
                inlines.push(inline);
            } else {
                break;
            }
        }

        trim_inlines(&mut inlines);
        inlines
    }

    // ── Math parsing ───────────────────────────────────────────

    fn parse_inline_math(&mut self) -> ast::Inline {
        let start = self.current_span();
        self.advance(); // consume $

        let node = self.parse_math_until_dollar();
        let end = self.current_span();

        ast::Inline::Math {
            node,
            span: start.merge(end),
        }
    }

    fn parse_math_until_dollar(&mut self) -> ast::MathNode {
        let mut children = Vec::new();
        let start = self.current_span();

        loop {
            if self.pos >= self.tokens.len() {
                break;
            }

            if matches!(
                self.peek(),
                Some(Token {
                    kind: TokenKind::Dollar,
                    ..
                })
            ) {
                self.advance(); // consume closing $
                break;
            }

            if let Some(node) = self.parse_math_atom() {
                children.push(node);
            } else {
                break;
            }
        }

        if children.len() == 1 {
            children.into_iter().next().unwrap_or(ast::MathNode::Group {
                children: Vec::new(),
                span: start,
            })
        } else {
            let end = self.current_span();
            ast::MathNode::Group {
                children,
                span: start.merge(end),
            }
        }
    }

    fn parse_math_until_dollar_dollar(&mut self) -> ast::MathNode {
        let mut children = Vec::new();
        let start = self.current_span();

        loop {
            if self.pos >= self.tokens.len() {
                break;
            }

            if matches!(
                self.peek(),
                Some(Token {
                    kind: TokenKind::DollarDollar,
                    ..
                })
            ) {
                self.advance(); // consume closing $$
                break;
            }

            if let Some(node) = self.parse_math_atom() {
                children.push(node);
            } else {
                break;
            }
        }

        if children.len() == 1 {
            children.into_iter().next().unwrap_or(ast::MathNode::Group {
                children: Vec::new(),
                span: start,
            })
        } else {
            let end = self.current_span();
            ast::MathNode::Group {
                children,
                span: start.merge(end),
            }
        }
    }

    fn parse_math_until_display_bracket(&mut self) -> ast::MathNode {
        let mut children = Vec::new();
        let start = self.current_span();

        loop {
            if self.pos >= self.tokens.len() {
                break;
            }

            if let Some(Token { kind: TokenKind::Command(name), .. }) = self.peek() {
                if name == "]" {
                    self.advance(); // consume \]
                    break;
                }
            }

            if let Some(node) = self.parse_math_atom() {
                children.push(node);
            } else {
                break;
            }
        }

        let end = self.current_span();
        ast::MathNode::Group {
            children,
            span: start.merge(end),
        }
    }

    fn parse_math_until_end_env(&mut self, _env_name: &str) -> ast::MathNode {
        let mut children = Vec::new();
        let start = self.current_span();

        loop {
            if self.pos >= self.tokens.len() {
                break;
            }

            // Check for \end{env_name}
            if self.is_command("end") {
                break;
            }

            // Skip whitespace/newlines in math mode
            if matches!(
                self.peek(),
                Some(Token {
                    kind: TokenKind::Newline | TokenKind::ParagraphBreak,
                    ..
                })
            ) {
                self.advance();
                continue;
            }

            if let Some(node) = self.parse_math_atom() {
                children.push(node);
            } else {
                break;
            }
        }

        let end = self.current_span();
        ast::MathNode::Group {
            children,
            span: start.merge(end),
        }
    }

    fn parse_math_atom(&mut self) -> Option<ast::MathNode> {
        let tok = self.peek()?;

        match &tok.kind {
            TokenKind::Word(w) => {
                let span = tok.span;
                let w = w.clone();
                self.advance();

                // In math mode, each letter is a separate identifier
                let node = if w.len() == 1 {
                    let ch = w.chars().next()?;
                    ast::MathNode::Atom {
                        char: ch,
                        class: classify_math_char(ch),
                        span,
                    }
                } else {
                    ast::MathNode::Ident { name: w, span }
                };

                // Check for super/subscript after
                Some(self.maybe_parse_scripts(node))
            }

            TokenKind::Number(n) => {
                let span = tok.span;
                let value = n.clone();
                self.advance();
                let node = ast::MathNode::Number { value, span };
                Some(self.maybe_parse_scripts(node))
            }

            TokenKind::Caret => {
                // Bare ^ without explicit base — use empty atom as base
                let span = tok.span;
                self.advance();
                self.skip_whitespace();
                let exp = self.parse_math_group_or_atom();
                let end = self.current_span();
                Some(ast::MathNode::Super {
                    base: Box::new(ast::MathNode::Group {
                        children: Vec::new(),
                        span,
                    }),
                    exp: Box::new(exp),
                    span: span.merge(end),
                })
            }

            TokenKind::Underscore => {
                let span = tok.span;
                self.advance();
                self.skip_whitespace();
                let sub = self.parse_math_group_or_atom();
                let end = self.current_span();
                Some(ast::MathNode::Sub {
                    base: Box::new(ast::MathNode::Group {
                        children: Vec::new(),
                        span,
                    }),
                    sub: Box::new(sub),
                    span: span.merge(end),
                })
            }

            TokenKind::BeginGroup => {
                let start = tok.span;
                self.advance(); // consume {
                let mut children = Vec::new();

                loop {
                    if self.pos >= self.tokens.len() {
                        break;
                    }
                    if matches!(
                        self.peek(),
                        Some(Token {
                            kind: TokenKind::EndGroup,
                            ..
                        })
                    ) {
                        self.advance();
                        break;
                    }
                    if let Some(node) = self.parse_math_atom() {
                        children.push(node);
                    } else {
                        break;
                    }
                }

                let end = self.current_span();
                let node = if children.len() == 1 {
                    children.into_iter().next()?
                } else {
                    ast::MathNode::Group {
                        children,
                        span: start.merge(end),
                    }
                };

                Some(self.maybe_parse_scripts(node))
            }

            TokenKind::Command(name) => {
                let name = name.clone();
                let start = tok.span;
                self.advance();

                let node = match name.as_str() {
                    "frac" => {
                        self.skip_whitespace();
                        let num = self.parse_math_group_or_atom();
                        self.skip_whitespace();
                        let den = self.parse_math_group_or_atom();
                        let end = self.current_span();
                        ast::MathNode::Frac {
                            num: Box::new(num),
                            den: Box::new(den),
                            span: start.merge(end),
                        }
                    }
                    "sqrt" => {
                        self.skip_whitespace();
                        let degree = if matches!(
                            self.peek(),
                            Some(Token {
                                kind: TokenKind::BracketOpen,
                                ..
                            })
                        ) {
                            if let Some((raw, _)) = self.read_bracket_group_raw() {
                                // Parse raw as math
                                Some(Box::new(ast::MathNode::Ident {
                                    name: raw,
                                    span: start,
                                }))
                            } else {
                                None
                            }
                        } else {
                            None
                        };
                        self.skip_whitespace();
                        let body = self.parse_math_group_or_atom();
                        let end = self.current_span();
                        ast::MathNode::Sqrt {
                            degree,
                            body: Box::new(body),
                            span: start.merge(end),
                        }
                    }
                    // Large operators
                    "sum" | "prod" | "coprod" | "bigcup" | "bigcap" | "bigoplus" | "bigotimes" => {
                        let node = ast::MathNode::LargeOp {
                            name: name.clone(),
                            limits: true,
                            span: start,
                        };
                        self.maybe_parse_scripts(node)
                    }
                    "int" | "oint" | "iint" | "iiint" => {
                        let node = ast::MathNode::LargeOp {
                            name: name.clone(),
                            limits: false,
                            span: start,
                        };
                        self.maybe_parse_scripts(node)
                    }
                    // Named operators
                    "sin" | "cos" | "tan" | "cot" | "sec" | "csc" | "arcsin" | "arccos"
                    | "arctan" | "sinh" | "cosh" | "tanh" | "log" | "ln" | "exp" | "lim"
                    | "limsup" | "liminf" | "sup" | "inf" | "min" | "max" | "det" | "gcd"
                    | "dim" | "ker" | "hom" | "deg" | "arg" | "Pr" | "mod" => {
                        let node = ast::MathNode::Operator {
                            name: name.clone(),
                            span: start,
                        };
                        self.maybe_parse_scripts(node)
                    }
                    // Math style commands
                    "mathbf" => self.parse_math_style(ast::MathStyle::Bold, start),
                    "mathit" => self.parse_math_style(ast::MathStyle::Italic, start),
                    "mathbb" => self.parse_math_style(ast::MathStyle::Blackboard, start),
                    "mathcal" => self.parse_math_style(ast::MathStyle::Calligraphic, start),
                    "mathsf" => self.parse_math_style(ast::MathStyle::SansSerif, start),
                    "mathtt" => self.parse_math_style(ast::MathStyle::Monospace, start),
                    "mathfrak" => self.parse_math_style(ast::MathStyle::Fraktur, start),
                    "mathrm" | "text" | "textrm" => {
                        self.skip_whitespace();
                        let content = self.parse_group_inlines();
                        let end = self.current_span();
                        ast::MathNode::Text {
                            content,
                            span: start.merge(end),
                        }
                    }
                    // Accents over
                    "hat" | "check" | "tilde" | "acute" | "grave" | "dot" | "ddot" | "breve"
                    | "bar" | "vec" | "widehat" | "widetilde" | "overline" | "overbrace" => {
                        self.skip_whitespace();
                        let body = self.parse_math_group_or_atom();
                        let end = self.current_span();
                        ast::MathNode::Over {
                            body: Box::new(body),
                            accent: name.clone(),
                            span: start.merge(end),
                        }
                    }
                    // Accents under
                    "underbrace" | "underline" => {
                        self.skip_whitespace();
                        let body = self.parse_math_group_or_atom();
                        let end = self.current_span();
                        ast::MathNode::Under {
                            body: Box::new(body),
                            accent: name.clone(),
                            span: start.merge(end),
                        }
                    }
                    // Delimiters
                    "left" => self.parse_left_right(start),
                    // Relations and binary ops as atoms
                    "leq" | "geq" | "neq" | "approx" | "equiv" | "sim" | "cong" | "propto"
                    | "subset" | "supset" | "subseteq" | "supseteq" | "in" | "notin" | "ni"
                    | "forall" | "exists" | "nexists" => {
                        let ch = command_to_unicode(&name);
                        let node = ast::MathNode::Atom {
                            char: ch,
                            class: ast::MathClass::Relation,
                            span: start,
                        };
                        self.maybe_parse_scripts(node)
                    }
                    "times" | "div" | "pm" | "mp" | "cdot" | "circ" | "bullet" | "oplus"
                    | "otimes" | "cup" | "cap" | "wedge" | "vee" | "setminus" => {
                        let ch = command_to_unicode(&name);
                        let node = ast::MathNode::Atom {
                            char: ch,
                            class: ast::MathClass::Binary,
                            span: start,
                        };
                        self.maybe_parse_scripts(node)
                    }
                    // Greek letters
                    "alpha" | "beta" | "gamma" | "delta" | "epsilon" | "zeta" | "eta" | "theta"
                    | "iota" | "kappa" | "lambda" | "mu" | "nu" | "xi" | "pi" | "rho" | "sigma"
                    | "tau" | "upsilon" | "phi" | "chi" | "psi" | "omega" | "varepsilon"
                    | "vartheta" | "varpi" | "varrho" | "varsigma" | "varphi" | "Gamma"
                    | "Delta" | "Theta" | "Lambda" | "Xi" | "Pi" | "Sigma" | "Upsilon" | "Phi"
                    | "Psi" | "Omega" => {
                        let ch = command_to_unicode(&name);
                        let node = ast::MathNode::Atom {
                            char: ch,
                            class: ast::MathClass::Ordinary,
                            span: start,
                        };
                        self.maybe_parse_scripts(node)
                    }
                    // Misc symbols
                    "infty" | "nabla" | "partial" | "ell" | "wp" | "Re" | "Im" | "aleph"
                    | "hbar" | "imath" | "jmath" | "emptyset" => {
                        let ch = command_to_unicode(&name);
                        let node = ast::MathNode::Atom {
                            char: ch,
                            class: ast::MathClass::Ordinary,
                            span: start,
                        };
                        self.maybe_parse_scripts(node)
                    }
                    // Arrows
                    "to" | "rightarrow" | "leftarrow" | "Rightarrow" | "Leftarrow"
                    | "leftrightarrow" | "Leftrightarrow" | "mapsto" | "hookrightarrow"
                    | "hookleftarrow" | "uparrow" | "downarrow" | "implies" | "iff" => {
                        let ch = command_to_unicode(&name);
                        let node = ast::MathNode::Atom {
                            char: ch,
                            class: ast::MathClass::Relation,
                            span: start,
                        };
                        self.maybe_parse_scripts(node)
                    }
                    "ldots" | "dots" | "cdots" | "vdots" | "ddots" => {
                        let ch = command_to_unicode(&name);
                        let node = ast::MathNode::Atom {
                            char: ch,
                            class: ast::MathClass::Inner,
                            span: start,
                        };
                        self.maybe_parse_scripts(node)
                    }
                    "quad" => ast::MathNode::Atom {
                        char: ' ',
                        class: ast::MathClass::Ordinary,
                        span: start,
                    },
                    "qquad" => ast::MathNode::Atom {
                        char: ' ',
                        class: ast::MathClass::Ordinary,
                        span: start,
                    },
                    // Spacing in math
                    "," | ";" | ":" | "!" => ast::MathNode::Atom {
                        char: ' ',
                        class: ast::MathClass::Ordinary,
                        span: start,
                    },
                    _ => {
                        // Unknown math command
                        ast::MathNode::Ident {
                            name: format!("\\{name}"),
                            span: start,
                        }
                    }
                };

                Some(node)
            }

            TokenKind::Whitespace => {
                self.advance();
                self.parse_math_atom()
            }

            TokenKind::Newline | TokenKind::ParagraphBreak => {
                self.advance();
                self.parse_math_atom()
            }

            TokenKind::Special(c) => {
                let ch = *c;
                let span = tok.span;
                self.advance();

                let class = classify_math_char(ch);
                let node = ast::MathNode::Atom {
                    char: ch,
                    class,
                    span,
                };
                Some(self.maybe_parse_scripts(node))
            }

            TokenKind::Backslash => {
                // \\ in math mode = row separator
                let span = tok.span;
                self.advance();
                Some(ast::MathNode::Row {
                    children: Vec::new(),
                    span,
                })
            }

            TokenKind::Ampersand => {
                // & in math = alignment tab
                let span = tok.span;
                self.advance();
                Some(ast::MathNode::Atom {
                    char: '&',
                    class: ast::MathClass::Ordinary,
                    span,
                })
            }

            TokenKind::Tilde => {
                let span = tok.span;
                self.advance();
                Some(ast::MathNode::Atom {
                    char: '~',
                    class: ast::MathClass::Ordinary,
                    span,
                })
            }

            // End tokens — return None
            TokenKind::Dollar | TokenKind::DollarDollar | TokenKind::EndGroup => None,

            _ => {
                self.advance();
                self.parse_math_atom()
            }
        }
    }

    fn maybe_parse_scripts(&mut self, base: ast::MathNode) -> ast::MathNode {
        self.skip_whitespace();

        let has_super = matches!(
            self.peek(),
            Some(Token {
                kind: TokenKind::Caret,
                ..
            })
        );
        let has_sub = matches!(
            self.peek(),
            Some(Token {
                kind: TokenKind::Underscore,
                ..
            })
        );

        if has_super {
            self.advance(); // consume ^
            self.skip_whitespace();
            let exp = self.parse_math_group_or_atom();
            let end = self.current_span();

            // Check for subscript after superscript
            self.skip_whitespace();
            if matches!(
                self.peek(),
                Some(Token {
                    kind: TokenKind::Underscore,
                    ..
                })
            ) {
                self.advance();
                self.skip_whitespace();
                let sub = self.parse_math_group_or_atom();
                let end2 = self.current_span();
                let span = base_span(&base).merge(end2);
                ast::MathNode::SubSuper {
                    base: Box::new(base),
                    sub: Box::new(sub),
                    sup: Box::new(exp),
                    span,
                }
            } else {
                let span = base_span(&base).merge(end);
                ast::MathNode::Super {
                    base: Box::new(base),
                    exp: Box::new(exp),
                    span,
                }
            }
        } else if has_sub {
            self.advance(); // consume _
            self.skip_whitespace();
            let sub = self.parse_math_group_or_atom();
            let end = self.current_span();

            // Check for superscript after subscript
            self.skip_whitespace();
            if matches!(
                self.peek(),
                Some(Token {
                    kind: TokenKind::Caret,
                    ..
                })
            ) {
                self.advance();
                self.skip_whitespace();
                let exp = self.parse_math_group_or_atom();
                let end2 = self.current_span();
                let span = base_span(&base).merge(end2);
                ast::MathNode::SubSuper {
                    base: Box::new(base),
                    sub: Box::new(sub),
                    sup: Box::new(exp),
                    span,
                }
            } else {
                let span = base_span(&base).merge(end);
                ast::MathNode::Sub {
                    base: Box::new(base),
                    sub: Box::new(sub),
                    span,
                }
            }
        } else {
            base
        }
    }

    fn parse_math_group_or_atom(&mut self) -> ast::MathNode {
        if matches!(
            self.peek(),
            Some(Token {
                kind: TokenKind::BeginGroup,
                ..
            })
        ) {
            let start = self.current_span();
            self.advance(); // consume {
            let mut children = Vec::new();

            loop {
                if self.pos >= self.tokens.len() {
                    break;
                }
                if matches!(
                    self.peek(),
                    Some(Token {
                        kind: TokenKind::EndGroup,
                        ..
                    })
                ) {
                    self.advance();
                    break;
                }
                if let Some(node) = self.parse_math_atom() {
                    children.push(node);
                } else {
                    break;
                }
            }

            let end = self.current_span();
            if children.len() == 1 {
                children.into_iter().next().unwrap_or(ast::MathNode::Group {
                    children: Vec::new(),
                    span: start.merge(end),
                })
            } else {
                ast::MathNode::Group {
                    children,
                    span: start.merge(end),
                }
            }
        } else {
            self.parse_math_atom().unwrap_or(ast::MathNode::Group {
                children: Vec::new(),
                span: self.current_span(),
            })
        }
    }

    fn parse_math_style(&mut self, style: ast::MathStyle, start: Span) -> ast::MathNode {
        self.skip_whitespace();
        let body = self.parse_math_group_or_atom();
        let end = self.current_span();
        ast::MathNode::Style {
            style,
            body: Box::new(body),
            span: start.merge(end),
        }
    }

    fn parse_left_right(&mut self, start: Span) -> ast::MathNode {
        self.skip_whitespace();
        // Parse left delimiter
        let _left_delim = self.parse_delimiter_char();

        let mut children = Vec::new();

        loop {
            if self.pos >= self.tokens.len() {
                break;
            }

            // Check for \right
            if self.is_command("right") {
                self.advance();
                self.skip_whitespace();
                let _right_delim = self.parse_delimiter_char();
                break;
            }

            if let Some(node) = self.parse_math_atom() {
                children.push(node);
            } else {
                break;
            }
        }

        let end = self.current_span();
        ast::MathNode::Group {
            children,
            span: start.merge(end),
        }
    }

    fn parse_delimiter_char(&mut self) -> Option<ast::DelimKind> {
        let tok = self.peek()?;
        let kind = match &tok.kind {
            TokenKind::Special('(') => Some(ast::DelimKind::LParen),
            TokenKind::Special(')') => Some(ast::DelimKind::RParen),
            TokenKind::BracketOpen => Some(ast::DelimKind::LBracket),
            TokenKind::BracketClose => Some(ast::DelimKind::RBracket),
            TokenKind::Special('.') => Some(ast::DelimKind::Dot),
            TokenKind::Special('|') => Some(ast::DelimKind::Vert),
            TokenKind::BeginGroup => Some(ast::DelimKind::LBrace),
            TokenKind::EndGroup => Some(ast::DelimKind::RBrace),
            TokenKind::Command(name) => match name.as_str() {
                "langle" => Some(ast::DelimKind::LAngle),
                "rangle" => Some(ast::DelimKind::RAngle),
                "lfloor" => Some(ast::DelimKind::LFloor),
                "rfloor" => Some(ast::DelimKind::RFloor),
                "lceil" => Some(ast::DelimKind::LCeil),
                "rceil" => Some(ast::DelimKind::RCeil),
                "lbrace" => Some(ast::DelimKind::LBrace),
                "rbrace" => Some(ast::DelimKind::RBrace),
                "|" => Some(ast::DelimKind::DoubleVert),
                "Vert" => Some(ast::DelimKind::DoubleVert),
                _ => None,
            },
            _ => None,
        };
        if kind.is_some() {
            self.advance();
        }
        kind
    }
}

// ── Helper functions ───────────────────────────────────────────

fn inlines_span(inlines: &[ast::Inline]) -> Span {
    if inlines.is_empty() {
        return Span::new(0, 0);
    }
    let first = inline_span(&inlines[0]);
    let last = inline_span(inlines.last().unwrap_or(&inlines[0]));
    first.merge(last)
}

fn inlines_span_or(inlines: &[ast::Inline], default: Span) -> Span {
    if inlines.is_empty() {
        default
    } else {
        inlines_span(inlines)
    }
}

fn inline_span(inline: &ast::Inline) -> Span {
    match inline {
        ast::Inline::Text { span, .. }
        | ast::Inline::Bold { span, .. }
        | ast::Inline::Italic { span, .. }
        | ast::Inline::BoldItalic { span, .. }
        | ast::Inline::Underline { span, .. }
        | ast::Inline::Monospace { span, .. }
        | ast::Inline::SmallCaps { span, .. }
        | ast::Inline::Emph { span, .. }
        | ast::Inline::Math { span, .. }
        | ast::Inline::Link { span, .. }
        | ast::Inline::Ref { span, .. }
        | ast::Inline::Citation { span, .. }
        | ast::Inline::FootnoteRef { span, .. }
        | ast::Inline::NonBreakingSpace { span }
        | ast::Inline::HSpace { span, .. }
        | ast::Inline::LineBreak { span }
        | ast::Inline::RawInlineCmd { span, .. } => *span,
    }
}

fn base_span(node: &ast::MathNode) -> Span {
    match node {
        ast::MathNode::Atom { span, .. }
        | ast::MathNode::Number { span, .. }
        | ast::MathNode::Ident { span, .. }
        | ast::MathNode::Operator { span, .. }
        | ast::MathNode::Frac { span, .. }
        | ast::MathNode::Sqrt { span, .. }
        | ast::MathNode::Super { span, .. }
        | ast::MathNode::Sub { span, .. }
        | ast::MathNode::SubSuper { span, .. }
        | ast::MathNode::Group { span, .. }
        | ast::MathNode::LargeOp { span, .. }
        | ast::MathNode::Delimiter { span, .. }
        | ast::MathNode::Over { span, .. }
        | ast::MathNode::Under { span, .. }
        | ast::MathNode::Text { span, .. }
        | ast::MathNode::Style { span, .. }
        | ast::MathNode::Row { span, .. }
        | ast::MathNode::Matrix { span, .. } => *span,
    }
}

fn trim_inlines(inlines: &mut Vec<ast::Inline>) {
    // Trim leading whitespace
    while let Some(first) = inlines.first() {
        if matches!(first, ast::Inline::Text { content, .. } if content.trim().is_empty()) {
            inlines.remove(0);
        } else {
            break;
        }
    }
    // Trim trailing whitespace
    while let Some(last) = inlines.last() {
        if matches!(last, ast::Inline::Text { content, .. } if content.trim().is_empty()) {
            inlines.pop();
        } else {
            break;
        }
    }
}

fn inlines_to_string(inlines: &[ast::Inline]) -> String {
    let mut result = String::new();
    for inline in inlines {
        match inline {
            ast::Inline::Text { content, .. } => result.push_str(content),
            ast::Inline::NonBreakingSpace { .. } => result.push('\u{00A0}'),
            _ => {}
        }
    }
    result
}

/// Parse a LaTeX length string to points.
fn parse_length_to_pt(s: &str) -> f64 {
    let s = s.trim();
    if s.is_empty() {
        return 0.0;
    }

    // Try to parse the numeric part
    let (num_str, unit) = if let Some(idx) = s.find(|c: char| c.is_ascii_alphabetic()) {
        (&s[..idx], s[idx..].trim())
    } else {
        (s, "pt")
    };

    let value: f64 = num_str.trim().parse().unwrap_or(0.0);

    match unit {
        "pt" => value,
        "bp" => value, // big points ≈ pt
        "mm" => value * 2.834_645_669_3,
        "cm" => value * 28.346_456_693,
        "in" => value * 72.0,
        "em" => value * 10.0, // approximate, depends on font size
        "ex" => value * 4.5,  // approximate
        "pc" => value * 12.0, // picas
        "sp" => value / 65536.0,
        _ => value, // assume pt
    }
}

fn classify_math_char(ch: char) -> ast::MathClass {
    match ch {
        '+' | '-' => ast::MathClass::Binary,
        '=' | '<' | '>' => ast::MathClass::Relation,
        '(' | '[' => ast::MathClass::Open,
        ')' | ']' => ast::MathClass::Close,
        ',' | ';' => ast::MathClass::Punct,
        _ => ast::MathClass::Ordinary,
    }
}

fn command_to_unicode(name: &str) -> char {
    match name {
        // Greek lowercase
        "alpha" => 'α',
        "beta" => 'β',
        "gamma" => 'γ',
        "delta" => 'δ',
        "epsilon" => 'ε',
        "varepsilon" => 'ε',
        "zeta" => 'ζ',
        "eta" => 'η',
        "theta" => 'θ',
        "vartheta" => 'ϑ',
        "iota" => 'ι',
        "kappa" => 'κ',
        "lambda" => 'λ',
        "mu" => 'μ',
        "nu" => 'ν',
        "xi" => 'ξ',
        "pi" => 'π',
        "varpi" => 'ϖ',
        "rho" => 'ρ',
        "varrho" => 'ϱ',
        "sigma" => 'σ',
        "varsigma" => 'ς',
        "tau" => 'τ',
        "upsilon" => 'υ',
        "phi" => 'φ',
        "varphi" => 'ϕ',
        "chi" => 'χ',
        "psi" => 'ψ',
        "omega" => 'ω',
        // Greek uppercase
        "Gamma" => 'Γ',
        "Delta" => 'Δ',
        "Theta" => 'Θ',
        "Lambda" => 'Λ',
        "Xi" => 'Ξ',
        "Pi" => 'Π',
        "Sigma" => 'Σ',
        "Upsilon" => 'Υ',
        "Phi" => 'Φ',
        "Psi" => 'Ψ',
        "Omega" => 'Ω',
        // Relations
        "leq" => '≤',
        "geq" => '≥',
        "neq" => '≠',
        "approx" => '≈',
        "equiv" => '≡',
        "sim" => '∼',
        "cong" => '≅',
        "propto" => '∝',
        "subset" => '⊂',
        "supset" => '⊃',
        "subseteq" => '⊆',
        "supseteq" => '⊇',
        "in" => '∈',
        "notin" => '∉',
        "ni" => '∋',
        "forall" => '∀',
        "exists" => '∃',
        "nexists" => '∄',
        // Binary ops
        "times" => '×',
        "div" => '÷',
        "pm" => '±',
        "mp" => '∓',
        "cdot" => '·',
        "circ" => '∘',
        "bullet" => '•',
        "oplus" => '⊕',
        "otimes" => '⊗',
        "cup" => '∪',
        "cap" => '∩',
        "wedge" => '∧',
        "vee" => '∨',
        "setminus" => '∖',
        // Misc
        "infty" => '∞',
        "nabla" => '∇',
        "partial" => '∂',
        "ell" => 'ℓ',
        "wp" => '℘',
        "Re" => 'ℜ',
        "Im" => 'ℑ',
        "aleph" => 'ℵ',
        "hbar" => 'ℏ',
        "emptyset" => '∅',
        "imath" => 'ı',
        "jmath" => 'ȷ',
        // Arrows
        "to" | "rightarrow" => '→',
        "leftarrow" => '←',
        "Rightarrow" | "implies" => '⇒',
        "Leftarrow" => '⇐',
        "leftrightarrow" => '↔',
        "Leftrightarrow" | "iff" => '⇔',
        "mapsto" => '↦',
        "hookrightarrow" => '↪',
        "hookleftarrow" => '↩',
        "uparrow" => '↑',
        "downarrow" => '↓',
        // Dots
        "ldots" | "dots" => '…',
        "cdots" => '⋯',
        "vdots" => '⋮',
        "ddots" => '⋱',
        _ => '?',
    }
}

fn apply_accent(accent: &str, base: &str) -> String {
    // Common accent combinations → precomposed Unicode
    match (accent, base) {
        ("'", "e") => "é".to_string(),
        ("'", "a") => "á".to_string(),
        ("'", "i") => "í".to_string(),
        ("'", "o") => "ó".to_string(),
        ("'", "u") => "ú".to_string(),
        ("'", "E") => "É".to_string(),
        ("`", "e") => "è".to_string(),
        ("`", "a") => "à".to_string(),
        ("`", "i") => "ì".to_string(),
        ("`", "o") => "ò".to_string(),
        ("`", "u") => "ù".to_string(),
        ("^", "e") => "ê".to_string(),
        ("^", "a") => "â".to_string(),
        ("^", "i") => "î".to_string(),
        ("^", "o") => "ô".to_string(),
        ("^", "u") => "û".to_string(),
        ("\"", "e") => "ë".to_string(),
        ("\"", "a") => "ä".to_string(),
        ("\"", "i") => "ï".to_string(),
        ("\"", "o") => "ö".to_string(),
        ("\"", "u") => "ü".to_string(),
        ("~", "n") => "ñ".to_string(),
        ("~", "a") => "ã".to_string(),
        ("~", "o") => "õ".to_string(),
        ("c", "c") => "ç".to_string(),
        ("c", "C") => "Ç".to_string(),
        _ => base.to_string(),
    }
}
