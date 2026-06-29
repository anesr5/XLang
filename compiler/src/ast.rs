use crate::diagnostic::Span;

#[derive(Debug, Clone, PartialEq)]
pub struct Program {
    pub module: Option<String>,
    pub imports: Vec<String>,
    pub structs: Vec<StructDecl>,
    pub functions: Vec<Function>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct StructDecl {
    pub name: String,
    pub name_span: Span,
    pub fields: Vec<Field>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Field {
    pub name: String,
    pub name_span: Span,
    pub ty: TypeName,
    pub ty_span: Span,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Function {
    pub name: String,
    pub name_span: Span,
    pub params: Vec<Param>,
    pub return_type: TypeName,
    pub return_type_span: Option<Span>,
    pub body: Vec<Stmt>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Param {
    pub name: String,
    pub name_span: Span,
    pub ty: TypeName,
    pub ty_span: Span,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TypeName {
    I32,
    Bool,
    Str,
    Void,
    Named(String),
}

#[derive(Debug, Clone, PartialEq)]
pub enum Stmt {
    Let {
        mutable: bool,
        name: String,
        name_span: Span,
        annotation: Option<TypeName>,
        annotation_span: Option<Span>,
        value: Expr,
    },
    Assign {
        name: String,
        name_span: Span,
        value: Expr,
    },
    Return {
        value: Option<Expr>,
        keyword_span: Span,
    },
    Expr(Expr),
    If {
        condition: Expr,
        keyword_span: Span,
        then_body: Vec<Stmt>,
        else_body: Vec<Stmt>,
    },
}

#[derive(Debug, Clone, PartialEq)]
pub enum Expr {
    Integer {
        value: i64,
        span: Span,
    },
    String {
        value: String,
        span: Span,
    },
    Bool {
        value: bool,
        span: Span,
    },
    Variable {
        name: String,
        span: Span,
    },
    Call {
        callee: String,
        args: Vec<Expr>,
        span: Span,
    },
    Unary {
        op: UnaryOp,
        expr: Box<Expr>,
        span: Span,
    },
    Binary {
        left: Box<Expr>,
        op: BinaryOp,
        right: Box<Expr>,
        span: Span,
    },
}

impl Expr {
    pub fn span(&self) -> Span {
        match self {
            Expr::Integer { span, .. }
            | Expr::String { span, .. }
            | Expr::Bool { span, .. }
            | Expr::Variable { span, .. }
            | Expr::Call { span, .. }
            | Expr::Unary { span, .. }
            | Expr::Binary { span, .. } => *span,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UnaryOp {
    Negate,
    Not,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BinaryOp {
    Add,
    Subtract,
    Multiply,
    Divide,
    Remainder,
    Equal,
    NotEqual,
    Less,
    LessEqual,
    Greater,
    GreaterEqual,
    And,
    Or,
}
