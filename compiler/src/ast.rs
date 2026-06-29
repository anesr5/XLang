use crate::diagnostic::Span;

#[derive(Debug, Clone, PartialEq)]
pub struct Program {
    pub module: Option<String>,
    pub imports: Vec<Import>,
    pub structs: Vec<StructDecl>,
    pub functions: Vec<Function>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Import {
    pub name: String,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq)]
pub struct StructDecl {
    pub pub_export: bool,
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
    pub pub_export: bool,
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
    Qualified { module: String, name: String },
    Array { elem: Box<TypeName>, len: usize },
}

impl TypeName {
    pub fn array_elem_len(&self) -> Option<(&TypeName, usize)> {
        match self {
            TypeName::Array { elem, len } => Some((elem.as_ref(), *len)),
            _ => None,
        }
    }

    pub fn struct_ref(&self) -> Option<StructRef<'_>> {
        match self {
            TypeName::Named(name) => Some(StructRef::Local(name)),
            TypeName::Qualified { module, name } => {
                Some(StructRef::Qualified { module, name })
            }
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StructRef<'a> {
    Local(&'a str),
    Qualified { module: &'a str, name: &'a str },
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
    While {
        condition: Expr,
        keyword_span: Span,
        body: Vec<Stmt>,
    },
    Break {
        keyword_span: Span,
    },
    Continue {
        keyword_span: Span,
    },
    AssignIndex {
        name: String,
        name_span: Span,
        index: Expr,
        value: Expr,
    },
    AssignField {
        name: String,
        name_span: Span,
        field: String,
        field_span: Span,
        value: Expr,
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
    QualifiedCall {
        module: String,
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
    ArrayLiteral {
        elements: Vec<Expr>,
        span: Span,
    },
    Index {
        base: Box<Expr>,
        index: Box<Expr>,
        span: Span,
    },
    StructLiteral {
        elements: Vec<Expr>,
        span: Span,
    },
    FieldAccess {
        base: Box<Expr>,
        field: String,
        field_span: Span,
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
            | Expr::QualifiedCall { span, .. }
            | Expr::Unary { span, .. }
            | Expr::Binary { span, .. }
            | Expr::ArrayLiteral { span, .. }
            | Expr::Index { span, .. }
            | Expr::StructLiteral { span, .. }
            | Expr::FieldAccess { span, .. } => *span,
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
