use std::collections::{HashMap, HashSet};

use crate::ast::{BinaryOp, Expr, Function, Program, Stmt, TypeName, UnaryOp};
use crate::diagnostic::{Diagnostic, Span, XResult};

#[derive(Debug, Clone)]
struct FunctionSig {
    params: Vec<TypeName>,
    first_param_span: Option<Span>,
    return_type: TypeName,
    return_type_span: Option<Span>,
    name_span: Span,
}

pub fn check(program: &Program) -> XResult<()> {
    TypeChecker::new(program)?.check_program(program)
}

struct TypeChecker {
    functions: HashMap<String, FunctionSig>,
}

impl TypeChecker {
    fn new(program: &Program) -> XResult<Self> {
        let mut functions = HashMap::new();
        for function in &program.functions {
            if functions.contains_key(&function.name) {
                return Err(Diagnostic::at(
                    format!("duplicate function `{}`", function.name),
                    function.name_span,
                ));
            }
            functions.insert(
                function.name.clone(),
                FunctionSig {
                    params: function.params.iter().map(|p| p.ty.clone()).collect(),
                    first_param_span: function.params.first().map(|p| p.name_span),
                    return_type: function.return_type.clone(),
                    return_type_span: function.return_type_span,
                    name_span: function.name_span,
                },
            );
        }
        if !functions.contains_key("main") {
            return Err(Diagnostic::new("program must define `fn main()`", 1, 1));
        }
        validate_main_signature(&functions)?;
        Ok(Self { functions })
    }

    fn check_program(&self, program: &Program) -> XResult<()> {
        for function in &program.functions {
            self.check_function(function)?;
        }
        Ok(())
    }

    fn check_function(&self, function: &Function) -> XResult<()> {
        ensure_supported_signature_type(
            &function.return_type,
            function.return_type_span.unwrap_or(function.name_span),
        )?;
        let mut locals = HashMap::new();
        let mut declared = HashSet::new();
        for param in &function.params {
            if !declared.insert(param.name.clone()) {
                return Err(Diagnostic::at(
                    format!("duplicate parameter `{}`", param.name),
                    param.name_span,
                ));
            }
            ensure_supported_signature_type(&param.ty, param.ty_span)?;
            if param.ty == TypeName::Void {
                return Err(Diagnostic::at(
                    format!("parameter `{}` cannot have type void", param.name),
                    param.ty_span,
                ));
            }
            locals.insert(param.name.clone(), (param.ty.clone(), true));
        }
        for stmt in &function.body {
            self.check_stmt(stmt, &mut locals, &mut declared, &function.return_type)?;
        }
        if function.return_type != TypeName::Void && !block_always_returns(&function.body) {
            return Err(Diagnostic::at(
                format!(
                    "function `{}` may exit without returning a value",
                    function.name
                ),
                function.name_span,
            ));
        }
        Ok(())
    }

    fn check_stmt(
        &self,
        stmt: &Stmt,
        locals: &mut HashMap<String, (TypeName, bool)>,
        declared: &mut HashSet<String>,
        expected_return: &TypeName,
    ) -> XResult<()> {
        match stmt {
            Stmt::Let {
                mutable,
                name,
                name_span,
                annotation,
                annotation_span,
                value,
            } => {
                let value_ty = self.check_expr(value, locals)?;
                if value_ty == TypeName::Void {
                    return Err(Diagnostic::at(
                        format!("cannot bind void value to `{name}`"),
                        value.span(),
                    ));
                }
                if let Some(annotation) = annotation
                    && annotation != &value_ty
                {
                    return Err(Diagnostic::at(
                        format!("cannot assign {:?} to {:?}", value_ty, annotation),
                        annotation_span.unwrap_or_else(|| value.span()),
                    ));
                }
                if declared.contains(name) {
                    return Err(Diagnostic::at(
                        format!("duplicate binding `{name}`"),
                        *name_span,
                    ));
                }
                declared.insert(name.clone());
                locals.insert(
                    name.clone(),
                    (annotation.clone().unwrap_or(value_ty), *mutable),
                );
            }
            Stmt::Assign {
                name,
                name_span,
                value,
            } => {
                let value_ty = self.check_expr(value, locals)?;
                let Some((target_ty, mutable)) = locals.get(name) else {
                    return Err(Diagnostic::at(
                        format!("unknown variable `{name}`"),
                        *name_span,
                    ));
                };
                if !*mutable {
                    return Err(Diagnostic::at(
                        format!("cannot assign to immutable binding `{name}`"),
                        *name_span,
                    ));
                }
                if target_ty != &value_ty {
                    return Err(Diagnostic::at(
                        format!("cannot assign {:?} to {:?}", value_ty, target_ty),
                        value.span(),
                    ));
                }
            }
            Stmt::Return {
                value,
                keyword_span,
            } => {
                let actual = if let Some(expr) = value {
                    let actual = self.check_expr(expr, locals)?;
                    if actual == TypeName::Void {
                        return Err(Diagnostic::at(
                            "cannot return a void expression; use `return;`",
                            expr.span(),
                        ));
                    }
                    actual
                } else {
                    TypeName::Void
                };
                if &actual != expected_return {
                    return Err(Diagnostic::at(
                        format!(
                            "return type mismatch: expected {:?}, got {:?}",
                            expected_return, actual
                        ),
                        value.as_ref().map(Expr::span).unwrap_or(*keyword_span),
                    ));
                }
            }
            Stmt::Expr(expr) => {
                self.check_expr(expr, locals)?;
            }
            Stmt::If {
                condition,
                keyword_span: _,
                then_body,
                else_body,
            } => {
                let condition_ty = self.check_expr(condition, locals)?;
                if condition_ty != TypeName::Bool {
                    return Err(Diagnostic::at(
                        "if condition must be bool",
                        condition.span(),
                    ));
                }
                let mut then_locals = locals.clone();
                for stmt in then_body {
                    self.check_stmt(stmt, &mut then_locals, declared, expected_return)?;
                }
                let mut else_locals = locals.clone();
                for stmt in else_body {
                    self.check_stmt(stmt, &mut else_locals, declared, expected_return)?;
                }
            }
        }
        Ok(())
    }

    fn check_expr(
        &self,
        expr: &Expr,
        locals: &HashMap<String, (TypeName, bool)>,
    ) -> XResult<TypeName> {
        match expr {
            Expr::Integer { value, span } => {
                ensure_i32_literal(*value, *span)?;
                Ok(TypeName::I32)
            }
            Expr::String { .. } => Ok(TypeName::Str),
            Expr::Bool { .. } => Ok(TypeName::Bool),
            Expr::Variable { name, span } => locals
                .get(name)
                .map(|(ty, _)| ty.clone())
                .ok_or_else(|| Diagnostic::at(format!("unknown variable `{name}`"), *span)),
            Expr::Call { callee, args, span } => self.check_call(callee, args, locals, *span),
            Expr::Unary { op, expr, span } => {
                if *op == UnaryOp::Negate
                    && let Expr::Integer { value, .. } = expr.as_ref()
                {
                    ensure_negated_i32_literal(*value, *span)?;
                    return Ok(TypeName::I32);
                }
                let ty = self.check_expr(expr, locals)?;
                match op {
                    UnaryOp::Negate if ty == TypeName::I32 => Ok(TypeName::I32),
                    UnaryOp::Not if ty == TypeName::Bool => Ok(TypeName::Bool),
                    UnaryOp::Negate => Err(Diagnostic::at("unary `-` requires i32", *span)),
                    UnaryOp::Not => Err(Diagnostic::at("unary `!` requires bool", *span)),
                }
            }
            Expr::Binary {
                left,
                op,
                right,
                span,
            } => {
                let left_ty = self.check_expr(left, locals)?;
                let right_ty = self.check_expr(right, locals)?;
                self.check_binary(*op, left_ty, right_ty, *span)
            }
        }
    }

    fn check_call(
        &self,
        callee: &str,
        args: &[Expr],
        locals: &HashMap<String, (TypeName, bool)>,
        span: crate::diagnostic::Span,
    ) -> XResult<TypeName> {
        let Some(sig) = self.functions.get(callee) else {
            return Err(Diagnostic::at(format!("unknown function `{callee}`"), span));
        };
        if sig.params.len() != args.len() {
            return Err(Diagnostic::at(
                format!(
                    "function `{callee}` expects {} arguments, got {}",
                    sig.params.len(),
                    args.len()
                ),
                span,
            ));
        }
        for (arg, expected) in args.iter().zip(sig.params.iter()) {
            let actual = self.check_expr(arg, locals)?;
            if actual == TypeName::Void {
                return Err(Diagnostic::at(
                    "cannot pass void expression as an argument",
                    arg.span(),
                ));
            }
            if &actual != expected {
                return Err(Diagnostic::at(
                    format!(
                        "argument type mismatch: expected {:?}, got {:?}",
                        expected, actual
                    ),
                    arg.span(),
                ));
            }
        }
        Ok(sig.return_type.clone())
    }

    fn check_binary(
        &self,
        op: BinaryOp,
        left: TypeName,
        right: TypeName,
        span: crate::diagnostic::Span,
    ) -> XResult<TypeName> {
        match op {
            BinaryOp::Add
            | BinaryOp::Subtract
            | BinaryOp::Multiply
            | BinaryOp::Divide
            | BinaryOp::Remainder => {
                if left == TypeName::I32 && right == TypeName::I32 {
                    Ok(TypeName::I32)
                } else {
                    Err(Diagnostic::at(
                        "arithmetic operators require i32 operands",
                        span,
                    ))
                }
            }
            BinaryOp::Less | BinaryOp::LessEqual | BinaryOp::Greater | BinaryOp::GreaterEqual => {
                if left == TypeName::I32 && right == TypeName::I32 {
                    Ok(TypeName::Bool)
                } else {
                    Err(Diagnostic::at(
                        "comparison operators require i32 operands",
                        span,
                    ))
                }
            }
            BinaryOp::Equal | BinaryOp::NotEqual => {
                if left == right {
                    Ok(TypeName::Bool)
                } else {
                    Err(Diagnostic::at(
                        "equality operands must have the same type",
                        span,
                    ))
                }
            }
            BinaryOp::And | BinaryOp::Or => {
                if left == TypeName::Bool && right == TypeName::Bool {
                    Ok(TypeName::Bool)
                } else {
                    Err(Diagnostic::at(
                        "logical operators require bool operands",
                        span,
                    ))
                }
            }
        }
    }
}

fn block_always_returns(stmts: &[Stmt]) -> bool {
    stmts.iter().any(stmt_always_returns)
}

fn stmt_always_returns(stmt: &Stmt) -> bool {
    match stmt {
        Stmt::Return { .. } => true,
        Stmt::If {
            then_body,
            else_body,
            ..
        } => {
            !else_body.is_empty()
                && block_always_returns(then_body)
                && block_always_returns(else_body)
        }
        Stmt::Let { .. } | Stmt::Assign { .. } | Stmt::Expr(_) => false,
    }
}

fn ensure_supported_signature_type(ty: &TypeName, span: Span) -> XResult<()> {
    match ty {
        TypeName::I32 | TypeName::Bool | TypeName::Str | TypeName::Void => Ok(()),
        TypeName::Named(name) => Err(Diagnostic::at(
            format!("struct type `{name}` is parsed but not supported in function signatures yet"),
            span,
        )),
    }
}

fn validate_main_signature(functions: &HashMap<String, FunctionSig>) -> XResult<()> {
    let main = functions
        .get("main")
        .expect("main existence is checked before signature validation");
    if !main.params.is_empty() {
        return Err(Diagnostic::at(
            "`main` must not have parameters",
            main.first_param_span.unwrap_or(main.name_span),
        ));
    }
    if main.return_type != TypeName::I32 {
        return Err(Diagnostic::at(
            "`main` must return i32 in the MVP",
            main.return_type_span.unwrap_or(main.name_span),
        ));
    }
    Ok(())
}

fn ensure_i32_literal(value: i64, span: crate::diagnostic::Span) -> XResult<()> {
    if value > i64::from(i32::MAX) {
        return Err(Diagnostic::at(
            format!("integer literal `{value}` does not fit in i32"),
            span,
        ));
    }
    Ok(())
}

fn ensure_negated_i32_literal(value: i64, span: crate::diagnostic::Span) -> XResult<()> {
    let min_magnitude = i64::from(i32::MAX) + 1;
    if value > min_magnitude {
        return Err(Diagnostic::at(
            format!("integer literal `-{value}` does not fit in i32"),
            span,
        ));
    }
    Ok(())
}
