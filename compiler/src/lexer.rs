use crate::diagnostic::Span;
use crate::diagnostic::{Diagnostic, XResult};
use crate::token::{Keyword, Token, TokenKind};

pub fn lex(source: &str) -> XResult<Vec<Token>> {
    Lexer::new(source).lex()
}

struct Lexer {
    chars: Vec<char>,
    index: usize,
    byte_index: usize,
    line: usize,
    column: usize,
}

impl Lexer {
    fn new(source: &str) -> Self {
        Self {
            chars: source.chars().collect(),
            index: 0,
            byte_index: 0,
            line: 1,
            column: 1,
        }
    }

    fn lex(mut self) -> XResult<Vec<Token>> {
        let mut tokens = Vec::new();
        while let Some(ch) = self.peek() {
            match ch {
                ' ' | '\t' | '\r' | '\n' => {
                    self.advance();
                }
                '/' if self.peek_next() == Some('/') => self.skip_line_comment(),
                '/' if self.peek_next() == Some('*') => self.skip_block_comment()?,
                '0'..='9' => tokens.push(self.lex_number()?),
                '"' => tokens.push(self.lex_string()?),
                '\'' => tokens.push(self.lex_char()?),
                'a'..='z' | 'A'..='Z' | '_' => tokens.push(self.lex_identifier()),
                '+' => tokens.push(self.single(TokenKind::Plus)),
                '-' if self.peek_next() == Some('>') => tokens.push(self.double(TokenKind::Arrow)),
                '-' => tokens.push(self.single(TokenKind::Minus)),
                '*' => tokens.push(self.single(TokenKind::Star)),
                '/' => tokens.push(self.single(TokenKind::Slash)),
                '%' => tokens.push(self.single(TokenKind::Percent)),
                '=' if self.peek_next() == Some('=') => {
                    tokens.push(self.double(TokenKind::EqualEqual))
                }
                '=' if self.peek_next() == Some('>') => {
                    tokens.push(self.double(TokenKind::FatArrow))
                }
                '=' => tokens.push(self.single(TokenKind::Equal)),
                '!' if self.peek_next() == Some('=') => {
                    tokens.push(self.double(TokenKind::BangEqual))
                }
                '!' => tokens.push(self.single(TokenKind::Bang)),
                '<' if self.peek_next() == Some('<') => {
                    tokens.push(self.double(TokenKind::LeftShift))
                }
                '<' if self.peek_next() == Some('=') => {
                    tokens.push(self.double(TokenKind::LessEqual))
                }
                '<' => tokens.push(self.single(TokenKind::Less)),
                '>' if self.peek_next() == Some('>') => {
                    tokens.push(self.double(TokenKind::RightShift))
                }
                '>' if self.peek_next() == Some('=') => {
                    tokens.push(self.double(TokenKind::GreaterEqual))
                }
                '>' => tokens.push(self.single(TokenKind::Greater)),
                '&' if self.peek_next() == Some('&') => tokens.push(self.double(TokenKind::AmpAmp)),
                '&' => tokens.push(self.single(TokenKind::Ampersand)),
                '|' if self.peek_next() == Some('|') => {
                    tokens.push(self.double(TokenKind::PipePipe))
                }
                '|' => tokens.push(self.single(TokenKind::Pipe)),
                '^' => tokens.push(self.single(TokenKind::Caret)),
                '~' => tokens.push(self.single(TokenKind::Tilde)),
                '.' => tokens.push(self.single(TokenKind::Dot)),
                ';' => tokens.push(self.single(TokenKind::Semicolon)),
                ',' => tokens.push(self.single(TokenKind::Comma)),
                ':' if self.peek_next() == Some(':') => {
                    tokens.push(self.double(TokenKind::ColonColon))
                }
                ':' => tokens.push(self.single(TokenKind::Colon)),
                '?' => tokens.push(self.single(TokenKind::Question)),
                '(' => tokens.push(self.single(TokenKind::LeftParen)),
                ')' => tokens.push(self.single(TokenKind::RightParen)),
                '{' => tokens.push(self.single(TokenKind::LeftBrace)),
                '}' => tokens.push(self.single(TokenKind::RightBrace)),
                '[' => tokens.push(self.single(TokenKind::LeftBracket)),
                ']' => tokens.push(self.single(TokenKind::RightBracket)),
                other => {
                    return Err(Diagnostic::lexical(
                        format!("unknown character `{other}`"),
                        self.line,
                        self.column,
                    ));
                }
            }
        }
        tokens.push(Token {
            kind: TokenKind::Eof,
            span: Span::new(
                self.byte_index,
                self.byte_index,
                self.line,
                self.column,
                self.line,
                self.column,
            ),
        });
        Ok(tokens)
    }

    fn skip_line_comment(&mut self) {
        while let Some(ch) = self.peek() {
            self.advance();
            if ch == '\n' {
                break;
            }
        }
    }

    fn skip_block_comment(&mut self) -> XResult<()> {
        let line = self.line;
        let column = self.column;
        self.advance();
        self.advance();
        while let Some(ch) = self.peek() {
            if ch == '*' && self.peek_next() == Some('/') {
                self.advance();
                self.advance();
                return Ok(());
            }
            self.advance();
        }
        Err(Diagnostic::lexical(
            "unterminated block comment",
            line,
            column,
        ))
    }

    fn lex_number(&mut self) -> XResult<Token> {
        let line = self.line;
        let column = self.column;
        let start_byte = self.byte_index;
        let mut text = String::new();
        while let Some(ch @ '0'..='9') = self.peek() {
            text.push(ch);
            self.advance();
        }
        if self.peek() == Some('.') && matches!(self.peek_n(2), Some('0'..='9')) {
            text.push('.');
            self.advance();
            while let Some(ch @ '0'..='9') = self.peek() {
                text.push(ch);
                self.advance();
            }
            return Ok(Token {
                kind: TokenKind::Float(text),
                span: self.span_from(start_byte, line, column),
            });
        }
        let value = text
            .parse::<i64>()
            .map_err(|_| Diagnostic::lexical("integer literal is too large", line, column))?;
        Ok(Token {
            kind: TokenKind::Integer(value),
            span: self.span_from(start_byte, line, column),
        })
    }

    fn lex_string(&mut self) -> XResult<Token> {
        let line = self.line;
        let column = self.column;
        let start_byte = self.byte_index;
        self.advance();
        let mut value = String::new();
        while let Some(ch) = self.peek() {
            if ch == '"' {
                self.advance();
                return Ok(Token {
                    kind: TokenKind::String(value),
                    span: self.span_from(start_byte, line, column),
                });
            }
            if ch == '\n' {
                return Err(Diagnostic::lexical(
                    "unterminated string literal",
                    line,
                    column,
                ));
            }
            value.push(self.lex_string_char(line, column)?);
        }
        Err(Diagnostic::lexical(
            "unterminated string literal",
            line,
            column,
        ))
    }

    fn lex_char(&mut self) -> XResult<Token> {
        let line = self.line;
        let column = self.column;
        let start_byte = self.byte_index;
        self.advance();
        let value = self.lex_string_char(line, column)?;
        if self.peek() != Some('\'') {
            return Err(Diagnostic::lexical(
                "invalid character literal",
                line,
                column,
            ));
        }
        self.advance();
        Ok(Token {
            kind: TokenKind::Char(value),
            span: self.span_from(start_byte, line, column),
        })
    }

    fn lex_string_char(&mut self, line: usize, column: usize) -> XResult<char> {
        let Some(ch) = self.peek() else {
            return Err(Diagnostic::lexical(
                "unterminated string literal",
                line,
                column,
            ));
        };
        if ch != '\\' {
            self.advance();
            return Ok(ch);
        }
        self.advance();
        let escaped = match self.peek() {
            Some('n') => '\n',
            Some('r') => '\r',
            Some('t') => '\t',
            Some('\\') => '\\',
            Some('"') => '"',
            Some('0') => '\0',
            Some(other) => {
                return Err(Diagnostic::lexical(
                    format!("unsupported escape sequence `\\{other}`"),
                    self.line,
                    self.column,
                ));
            }
            None => {
                return Err(Diagnostic::lexical(
                    "unterminated string literal",
                    line,
                    column,
                ));
            }
        };
        self.advance();
        Ok(escaped)
    }

    fn lex_identifier(&mut self) -> Token {
        let line = self.line;
        let column = self.column;
        let start_byte = self.byte_index;
        let mut text = String::new();
        while let Some(ch) = self.peek() {
            if ch.is_ascii_alphanumeric() || ch == '_' {
                text.push(ch);
                self.advance();
            } else {
                break;
            }
        }
        let kind = match text.as_str() {
            "module" => TokenKind::Keyword(Keyword::Module),
            "import" => TokenKind::Keyword(Keyword::Import),
            "struct" => TokenKind::Keyword(Keyword::Struct),
            "enum" => TokenKind::Keyword(Keyword::Enum),
            "trait" => TokenKind::Keyword(Keyword::Trait),
            "const" => TokenKind::Keyword(Keyword::Const),
            "return" => TokenKind::Keyword(Keyword::Return),
            "if" => TokenKind::Keyword(Keyword::If),
            "else" => TokenKind::Keyword(Keyword::Else),
            "match" => TokenKind::Keyword(Keyword::Match),
            "for" => TokenKind::Keyword(Keyword::For),
            "while" => TokenKind::Keyword(Keyword::While),
            "loop" => TokenKind::Keyword(Keyword::Loop),
            "break" => TokenKind::Keyword(Keyword::Break),
            "continue" => TokenKind::Keyword(Keyword::Continue),
            "defer" => TokenKind::Keyword(Keyword::Defer),
            "async" => TokenKind::Keyword(Keyword::Async),
            "await" => TokenKind::Keyword(Keyword::Await),
            "parallel" => TokenKind::Keyword(Keyword::Parallel),
            "spawn" => TokenKind::Keyword(Keyword::Spawn),
            "gpu" => TokenKind::Keyword(Keyword::Gpu),
            "unsafe" => TokenKind::Keyword(Keyword::Unsafe),
            "pub" => TokenKind::Keyword(Keyword::Pub),
            "impl" => TokenKind::Keyword(Keyword::Impl),
            "where" => TokenKind::Keyword(Keyword::Where),
            "static" => TokenKind::Keyword(Keyword::Static),
            "type" => TokenKind::Keyword(Keyword::Type),
            "sizeof" => TokenKind::Keyword(Keyword::Sizeof),
            "alignof" => TokenKind::Keyword(Keyword::Alignof),
            "move" => TokenKind::Keyword(Keyword::Move),
            "mut" => TokenKind::Keyword(Keyword::Mut),
            "as" => TokenKind::Keyword(Keyword::As),
            "in" => TokenKind::Keyword(Keyword::In),
            "true" => TokenKind::Keyword(Keyword::True),
            "false" => TokenKind::Keyword(Keyword::False),
            _ => TokenKind::Identifier(text),
        };
        Token {
            kind,
            span: self.span_from(start_byte, line, column),
        }
    }

    fn single(&mut self, kind: TokenKind) -> Token {
        let start_byte = self.byte_index;
        let line = self.line;
        let column = self.column;
        self.advance();
        Token {
            kind,
            span: self.span_from(start_byte, line, column),
        }
    }

    fn double(&mut self, kind: TokenKind) -> Token {
        let start_byte = self.byte_index;
        let line = self.line;
        let column = self.column;
        self.advance();
        self.advance();
        Token {
            kind,
            span: self.span_from(start_byte, line, column),
        }
    }

    fn peek(&self) -> Option<char> {
        self.chars.get(self.index).copied()
    }

    fn peek_next(&self) -> Option<char> {
        self.chars.get(self.index + 1).copied()
    }

    fn peek_n(&self, offset: usize) -> Option<char> {
        self.chars.get(self.index + offset).copied()
    }

    fn advance(&mut self) -> Option<char> {
        let ch = self.peek()?;
        self.index += 1;
        self.byte_index += ch.len_utf8();
        if ch == '\n' {
            self.line += 1;
            self.column = 1;
        } else {
            self.column += 1;
        }
        Some(ch)
    }

    fn span_from(&self, start_byte: usize, start_line: usize, start_column: usize) -> Span {
        Span::new(
            start_byte,
            self.byte_index,
            start_line,
            start_column,
            self.line,
            self.column,
        )
    }
}
