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
        validate_structs(program)?;
        let mut functions = HashMap::new();
        for function in &program.functions {
            if functions.contains_key(&function.name) {
                return Err(Diagnostic::type_error_at(
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
            return Err(Diagnostic::type_error("program must define `main()`", 1, 1));
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
                return Err(Diagnostic::type_error_at(
                    format!("duplicate parameter `{}`", param.name),
                    param.name_span,
                ));
            }
            ensure_supported_signature_type(&param.ty, param.ty_span)?;
            if param.ty == TypeName::Void {
                return Err(Diagnostic::type_error_at(
                    format!("parameter `{}` cannot have type void", param.name),
                    param.ty_span,
                ));
            }
            locals.insert(param.name.clone(), (param.ty.clone(), true));
        }
        self.check_block(
            &function.body,
            &mut locals,
            &mut declared,
            &function.return_type,
            0,
        )?;
        if function.return_type != TypeName::Void && !block_always_returns(&function.body) {
            return Err(Diagnostic::type_error_at(
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
        loop_depth: usize,
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
                if matches!(annotation, Some(TypeName::Void)) {
                    return Err(Diagnostic::type_error_at(
                        format!("local binding `{name}` cannot have type void"),
                        annotation_span.unwrap_or(*name_span),
                    ));
                }
                if let Some(TypeName::Named(type_name)) = annotation {
                    return Err(Diagnostic::type_error_at(
                        format!(
                            "struct type `{type_name}` is parsed but not supported for local values yet"
                        ),
                        annotation_span.unwrap_or(*name_span),
                    ));
                }
                if let Some(array_ty) = annotation {
                    validate_array_local_type(array_ty, annotation_span.unwrap_or(*name_span))?;
                }
                let value_ty = self.check_expr(value, locals)?;
                if value_ty == TypeName::Void {
                    return Err(Diagnostic::type_error_at(
                        format!("cannot bind void value to `{name}`"),
                        value.span(),
                    ));
                }
                if let Some(annotation) = annotation
                    && annotation != &value_ty
                {
                    if let (Some((_, expected_len)), TypeName::Array { len, .. }) =
                        (annotation.array_elem_len(), &value_ty)
                        && expected_len != *len
                    {
                        return Err(Diagnostic::type_error_at(
                            format!(
                                "array literal length mismatch: expected {expected_len} elements, got {len}"
                            ),
                            value.span(),
                        ));
                    }
                    return Err(Diagnostic::type_error_at(
                        format!("cannot assign {:?} to {:?}", value_ty, annotation),
                        annotation_span.unwrap_or_else(|| value.span()),
                    ));
                }
                if declared.contains(name) {
                    return Err(Diagnostic::type_error_at(
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
                    return Err(Diagnostic::type_error_at(
                        format!("unknown variable `{name}`"),
                        *name_span,
                    ));
                };
                if !*mutable {
                    return Err(Diagnostic::type_error_at(
                        format!("cannot assign to immutable binding `{name}`"),
                        *name_span,
                    ));
                }
                if target_ty != &value_ty {
                    return Err(Diagnostic::type_error_at(
                        format!("cannot assign {:?} to {:?}", value_ty, target_ty),
                        value.span(),
                    ));
                }
            }
            Stmt::AssignIndex {
                name,
                name_span,
                index,
                value,
            } => {
                let Some((target_ty, mutable)) = locals.get(name) else {
                    return Err(Diagnostic::type_error_at(
                        format!("unknown variable `{name}`"),
                        *name_span,
                    ));
                };
                if !*mutable {
                    return Err(Diagnostic::type_error_at(
                        format!("cannot assign through const array binding `{name}`"),
                        *name_span,
                    ));
                }
                let Some((elem_ty, len)) = target_ty.array_elem_len() else {
                    return Err(Diagnostic::type_error_at(
                        format!(
                            "cannot index value of type {}",
                            type_name_display(target_ty)
                        ),
                        *name_span,
                    ));
                };
                let index_ty = self.check_expr(index, locals)?;
                if index_ty != TypeName::I32 {
                    return Err(Diagnostic::type_error_at(
                        "array index must be i32",
                        index.span(),
                    ));
                }
                check_constant_index_in_bounds(index, len)?;
                let value_ty = self.check_expr(value, locals)?;
                if &value_ty != elem_ty {
                    return Err(Diagnostic::type_error_at(
                        format!("cannot assign {:?} to {:?}", value_ty, elem_ty),
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
                        return Err(Diagnostic::type_error_at(
                            "cannot return a void expression; use `return;`",
                            expr.span(),
                        ));
                    }
                    actual
                } else {
                    TypeName::Void
                };
                if &actual != expected_return {
                    return Err(Diagnostic::type_error_at(
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
                    return Err(Diagnostic::type_error_at(
                        "if condition must be bool",
                        condition.span(),
                    ));
                }
                let mut then_locals = locals.clone();
                self.check_block(
                    then_body,
                    &mut then_locals,
                    declared,
                    expected_return,
                    loop_depth,
                )?;
                let mut else_locals = locals.clone();
                self.check_block(
                    else_body,
                    &mut else_locals,
                    declared,
                    expected_return,
                    loop_depth,
                )?;
            }
            Stmt::While {
                condition,
                keyword_span: _,
                body,
            } => {
                let condition_ty = self.check_expr(condition, locals)?;
                if condition_ty != TypeName::Bool {
                    return Err(Diagnostic::type_error_at(
                        format!("while condition must be bool, got {condition_ty:?}"),
                        condition.span(),
                    ));
                }
                let mut body_locals = locals.clone();
                self.check_block(
                    body,
                    &mut body_locals,
                    declared,
                    expected_return,
                    loop_depth + 1,
                )?;
            }
            Stmt::Break { keyword_span } => {
                if loop_depth == 0 {
                    return Err(Diagnostic::type_error_at(
                        "break outside of loop",
                        *keyword_span,
                    ));
                }
            }
            Stmt::Continue { keyword_span } => {
                if loop_depth == 0 {
                    return Err(Diagnostic::type_error_at(
                        "continue outside of loop",
                        *keyword_span,
                    ));
                }
            }
        }
        Ok(())
    }

    fn check_block(
        &self,
        stmts: &[Stmt],
        locals: &mut HashMap<String, (TypeName, bool)>,
        declared: &mut HashSet<String>,
        expected_return: &TypeName,
        loop_depth: usize,
    ) -> XResult<()> {
        let mut terminated = false;
        for stmt in stmts {
            if terminated {
                return Err(Diagnostic::type_error_at(
                    "unreachable statement after return",
                    stmt_anchor_span(stmt),
                ));
            }
            self.check_stmt(stmt, locals, declared, expected_return, loop_depth)?;
            terminated = stmt_cuts_off_fallthrough(stmt);
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
            Expr::Variable { name, span } => {
                locals.get(name).map(|(ty, _)| ty.clone()).ok_or_else(|| {
                    Diagnostic::type_error_at(format!("unknown variable `{name}`"), *span)
                })
            }
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
                    UnaryOp::Negate => {
                        Err(Diagnostic::type_error_at("unary `-` requires i32", *span))
                    }
                    UnaryOp::Not => {
                        Err(Diagnostic::type_error_at("unary `!` requires bool", *span))
                    }
                }
            }
            Expr::Binary {
                left,
                op,
                right,
                span,
            } => {
                if matches!(op, BinaryOp::Divide | BinaryOp::Remainder)
                    && let Some(zero_span) = zero_integer_literal_span(right)
                {
                    return Err(Diagnostic::type_error_at(
                        "division or remainder by zero",
                        zero_span,
                    ));
                }
                let left_ty = self.check_expr(left, locals)?;
                let right_ty = self.check_expr(right, locals)?;
                self.check_binary(*op, left_ty, right_ty, *span)
            }
            Expr::ArrayLiteral { elements, span } => {
                if elements.is_empty() {
                    return Err(Diagnostic::type_error_at(
                        "array literal must contain at least one element",
                        *span,
                    ));
                }
                let first_ty = self.check_expr(&elements[0], locals)?;
                for element in elements.iter().skip(1) {
                    let elem_ty = self.check_expr(element, locals)?;
                    if elem_ty != first_ty {
                        return Err(Diagnostic::type_error_at(
                            format!(
                                "array element type mismatch: expected {first_ty:?}, got {elem_ty:?}"
                            ),
                            element.span(),
                        ));
                    }
                }
                Ok(TypeName::Array {
                    elem: Box::new(first_ty),
                    len: elements.len(),
                })
            }
            Expr::Index {
                base,
                index,
                span: _,
            } => {
                let base_ty = self.check_expr(base, locals)?;
                let Some((elem_ty, len)) = base_ty.array_elem_len() else {
                    return Err(Diagnostic::type_error_at(
                        format!("cannot index value of type {}", type_name_display(&base_ty)),
                        base.span(),
                    ));
                };
                let index_ty = self.check_expr(index, locals)?;
                if index_ty != TypeName::I32 {
                    return Err(Diagnostic::type_error_at(
                        "array index must be i32",
                        index.span(),
                    ));
                }
                check_constant_index_in_bounds(index, len)?;
                Ok(elem_ty.clone())
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
            return Err(Diagnostic::type_error_at(
                format!("unknown function `{callee}`"),
                span,
            ));
        };
        if sig.params.len() != args.len() {
            return Err(Diagnostic::type_error_at(
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
                return Err(Diagnostic::type_error_at(
                    "cannot pass void expression as an argument",
                    arg.span(),
                ));
            }
            if &actual != expected {
                return Err(Diagnostic::type_error_at(
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
                    Err(Diagnostic::type_error_at(
                        "arithmetic operators require i32 operands",
                        span,
                    ))
                }
            }
            BinaryOp::Less | BinaryOp::LessEqual | BinaryOp::Greater | BinaryOp::GreaterEqual => {
                if left == TypeName::I32 && right == TypeName::I32 {
                    Ok(TypeName::Bool)
                } else {
                    Err(Diagnostic::type_error_at(
                        "comparison operators require i32 operands",
                        span,
                    ))
                }
            }
            BinaryOp::Equal | BinaryOp::NotEqual => {
                if left == right {
                    Ok(TypeName::Bool)
                } else {
                    Err(Diagnostic::type_error_at(
                        "equality operands must have the same type",
                        span,
                    ))
                }
            }
            BinaryOp::And | BinaryOp::Or => {
                if left == TypeName::Bool && right == TypeName::Bool {
                    Ok(TypeName::Bool)
                } else {
                    Err(Diagnostic::type_error_at(
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
        Stmt::Let { .. }
        | Stmt::Assign { .. }
        | Stmt::AssignIndex { .. }
        | Stmt::Expr(_)
        | Stmt::While { .. }
        | Stmt::Break { .. }
        | Stmt::Continue { .. } => false,
    }
}

fn stmt_cuts_off_fallthrough(stmt: &Stmt) -> bool {
    matches!(
        stmt,
        Stmt::Return { .. } | Stmt::Break { .. } | Stmt::Continue { .. }
    )
}

fn stmt_anchor_span(stmt: &Stmt) -> Span {
    match stmt {
        Stmt::Let {
            annotation_span,
            name_span,
            ..
        } => annotation_span.unwrap_or(*name_span),
        Stmt::Assign { name_span, .. } | Stmt::AssignIndex { name_span, .. } => *name_span,
        Stmt::Return { keyword_span, .. } => *keyword_span,
        Stmt::Expr(expr) => expr.span(),
        Stmt::If { keyword_span, .. } | Stmt::While { keyword_span, .. } => *keyword_span,
        Stmt::Break { keyword_span } | Stmt::Continue { keyword_span } => *keyword_span,
    }
}

const MAX_STACK_ARRAY_LEN: usize = 65_535;

fn validate_array_local_type(ty: &TypeName, span: Span) -> XResult<()> {
    let Some((elem, len)) = ty.array_elem_len() else {
        return Ok(());
    };
    if len == 0 {
        return Err(Diagnostic::type_error_at(
            "array length must be at least 1",
            span,
        ));
    }
    if len > MAX_STACK_ARRAY_LEN {
        return Err(Diagnostic::type_error_at(
            format!("array length {len} exceeds maximum stack array size {MAX_STACK_ARRAY_LEN}"),
            span,
        ));
    }
    match elem {
        TypeName::I32 | TypeName::Bool => Ok(()),
        TypeName::Str => Err(Diagnostic::type_error_at(
            "array element type str is not supported by the LLVM backend yet",
            span,
        )),
        TypeName::Void => Err(Diagnostic::type_error_at(
            "array element type cannot be void",
            span,
        )),
        TypeName::Named(name) => Err(Diagnostic::type_error_at(
            format!("array element type `{name}` is not supported yet"),
            span,
        )),
        TypeName::Array { .. } => Err(Diagnostic::type_error_at(
            "nested arrays are not supported in v0.2",
            span,
        )),
    }
}

fn check_constant_index_in_bounds(index: &Expr, len: usize) -> XResult<()> {
    if let Expr::Integer { value, span } = index
        && (*value < 0 || (*value as usize) >= len)
    {
        return Err(Diagnostic::type_error_at(
            format!("index {value} is out of bounds for array of length {len}"),
            *span,
        ));
    }
    Ok(())
}

fn type_name_display(ty: &TypeName) -> String {
    match ty {
        TypeName::I32 => "I32".to_owned(),
        TypeName::Bool => "Bool".to_owned(),
        TypeName::Str => "Str".to_owned(),
        TypeName::Void => "Void".to_owned(),
        TypeName::Named(name) => name.clone(),
        TypeName::Array { elem, len } => format!("{}[{len}]", type_name_display(elem)),
    }
}

fn validate_structs(program: &Program) -> XResult<()> {
    let mut struct_names = HashSet::new();
    for struct_decl in &program.structs {
        if !struct_names.insert(struct_decl.name.clone()) {
            return Err(Diagnostic::type_error_at(
                format!("duplicate struct `{}`", struct_decl.name),
                struct_decl.name_span,
            ));
        }

        let mut field_names = HashSet::new();
        for field in &struct_decl.fields {
            if !field_names.insert(field.name.clone()) {
                return Err(Diagnostic::type_error_at(
                    format!(
                        "duplicate field `{}` in struct `{}`",
                        field.name, struct_decl.name
                    ),
                    field.name_span,
                ));
            }
        }
    }
    Ok(())
}

fn ensure_supported_signature_type(ty: &TypeName, span: Span) -> XResult<()> {
    match ty {
        TypeName::I32 | TypeName::Bool | TypeName::Str | TypeName::Void => Ok(()),
        TypeName::Array { .. } => Err(Diagnostic::type_error_at(
            "array type not supported in function signatures yet",
            span,
        )),
        TypeName::Named(name) => Err(Diagnostic::type_error_at(
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
        return Err(Diagnostic::type_error_at(
            "`main` must not have parameters",
            main.first_param_span.unwrap_or(main.name_span),
        ));
    }
    if main.return_type != TypeName::I32 {
        return Err(Diagnostic::type_error_at(
            "`main` must return i32 in the MVP",
            main.return_type_span.unwrap_or(main.name_span),
        ));
    }
    Ok(())
}

fn ensure_i32_literal(value: i64, span: crate::diagnostic::Span) -> XResult<()> {
    if value > i64::from(i32::MAX) {
        return Err(Diagnostic::type_error_at(
            format!("integer literal `{value}` does not fit in i32"),
            span,
        ));
    }
    Ok(())
}

fn ensure_negated_i32_literal(value: i64, span: crate::diagnostic::Span) -> XResult<()> {
    let min_magnitude = i64::from(i32::MAX) + 1;
    if value > min_magnitude {
        return Err(Diagnostic::type_error_at(
            format!("integer literal `-{value}` does not fit in i32"),
            span,
        ));
    }
    Ok(())
}

fn zero_integer_literal_span(expr: &Expr) -> Option<Span> {
    match expr {
        Expr::Integer { value: 0, span } => Some(*span),
        Expr::Unary {
            op: UnaryOp::Negate,
            expr,
            span,
        } if matches!(expr.as_ref(), Expr::Integer { value: 0, .. }) => Some(*span),
        _ => None,
    }
}
