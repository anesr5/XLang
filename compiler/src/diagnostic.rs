use std::path::Path;

pub type XResult<T> = Result<T, Diagnostic>;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Diagnostic {
    pub message: String,
    pub span: Span,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Span {
    pub file_id: u32,
    pub start_byte: usize,
    pub end_byte: usize,
    pub start_line: usize,
    pub start_column: usize,
    pub end_line: usize,
    pub end_column: usize,
}

impl Diagnostic {
    pub fn new(message: impl Into<String>, line: usize, column: usize) -> Self {
        Self {
            message: message.into(),
            span: Span::point(line, column),
        }
    }

    pub fn at(message: impl Into<String>, span: Span) -> Self {
        Self {
            message: message.into(),
            span,
        }
    }

    pub fn render(&self, file: &Path) -> String {
        format!(
            "error: {}\n --> {}:{}:{}",
            self.message,
            file.display(),
            self.span.start_line,
            self.span.start_column
        )
    }
}

impl Span {
    pub fn point(line: usize, column: usize) -> Self {
        Self {
            file_id: 0,
            start_byte: 0,
            end_byte: 0,
            start_line: line,
            start_column: column,
            end_line: line,
            end_column: column,
        }
    }

    pub fn new(
        start_byte: usize,
        end_byte: usize,
        start_line: usize,
        start_column: usize,
        end_line: usize,
        end_column: usize,
    ) -> Self {
        Self {
            file_id: 0,
            start_byte,
            end_byte,
            start_line,
            start_column,
            end_line,
            end_column,
        }
    }

    pub fn join(self, other: Self) -> Self {
        Self {
            file_id: self.file_id,
            start_byte: self.start_byte,
            end_byte: other.end_byte,
            start_line: self.start_line,
            start_column: self.start_column,
            end_line: other.end_line,
            end_column: other.end_column,
        }
    }
}
