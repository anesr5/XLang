use crate::diagnostic::Span;

#[derive(Debug, Clone, PartialEq)]
pub enum TokenKind {
    Identifier(String),
    Integer(i64),
    Float(String),
    String(String),
    Char(char),
    Keyword(Keyword),
    Plus,
    Minus,
    Star,
    Slash,
    Percent,
    Equal,
    EqualEqual,
    Bang,
    BangEqual,
    Less,
    LessEqual,
    Greater,
    GreaterEqual,
    Ampersand,
    AmpAmp,
    Pipe,
    PipePipe,
    Caret,
    Tilde,
    LeftShift,
    RightShift,
    Arrow,
    FatArrow,
    Dot,
    Semicolon,
    Comma,
    Colon,
    ColonColon,
    Question,
    LeftParen,
    RightParen,
    LeftBrace,
    RightBrace,
    LeftBracket,
    RightBracket,
    Eof,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Keyword {
    Module,
    Import,
    Fn,
    Struct,
    Enum,
    Trait,
    Let,
    Var,
    Const,
    Return,
    If,
    Else,
    Match,
    For,
    While,
    Loop,
    Break,
    Continue,
    Defer,
    Async,
    Await,
    Parallel,
    Spawn,
    Gpu,
    Unsafe,
    Pub,
    Impl,
    Where,
    Static,
    Type,
    Sizeof,
    Alignof,
    Move,
    Mut,
    As,
    In,
    True,
    False,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Token {
    pub kind: TokenKind,
    pub span: Span,
}

impl Token {
    pub fn line(&self) -> usize {
        self.span.start_line
    }

    pub fn column(&self) -> usize {
        self.span.start_column
    }
}
