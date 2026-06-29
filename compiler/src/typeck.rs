use std::collections::{HashMap, HashSet};

use crate::ast::{
    BinaryOp, EnumRef, Expr, Function, MatchArm, MatchBody, Pattern, Program, Stmt, StructRef,
    TypeName, UnaryOp,
};
use crate::diagnostic::{Diagnostic, Span, XResult};
use crate::modules::{CompilationUnit, LoadedModule};

#[derive(Debug, Clone)]
struct FunctionSig {
    params: Vec<TypeName>,
    first_param_span: Option<Span>,
    return_type: TypeName,
    return_type_span: Option<Span>,
    name_span: Span,
    pub_export: bool,
}

#[derive(Debug, Clone)]
struct StructLayout {
    fields: Vec<(String, TypeName)>,
    pub_export: bool,
}

#[derive(Debug, Clone)]
struct EnumLayout {
    variants: Vec<(String, Option<TypeName>)>,
    pub_export: bool,
}

#[derive(Debug, Clone)]
struct ModuleExports {
    functions: HashMap<String, FunctionSig>,
    structs: HashMap<String, StructLayout>,
    enums: HashMap<String, EnumLayout>,
}

pub fn check_unit(unit: &CompilationUnit) -> XResult<()> {
    let exports = build_unit_exports(unit)?;
    validate_main_across_unit(unit, &exports)?;
    for module in unit.modules.values() {
        TypeChecker::check_module(unit, &exports, module)?;
    }
    Ok(())
}

pub fn check(program: &Program) -> XResult<()> {
    check_unit(&CompilationUnit::from_program(program.clone())?)
}

struct TypeChecker<'a> {
    module_name: &'a str,
    imports: HashSet<String>,
    exports: &'a HashMap<String, ModuleExports>,
    functions: HashMap<String, FunctionSig>,
    structs: HashMap<String, StructLayout>,
    enums: HashMap<String, EnumLayout>,
}

impl<'a> TypeChecker<'a> {
    fn check_module(
        unit: &'a CompilationUnit,
        exports: &'a HashMap<String, ModuleExports>,
        module: &'a LoadedModule,
    ) -> XResult<()> {
        let checker = Self::new(unit, exports, module)?;
        checker.check_program(&module.program)
    }

    fn new(
        unit: &'a CompilationUnit,
        exports: &'a HashMap<String, ModuleExports>,
        module: &'a LoadedModule,
    ) -> XResult<Self> {
        let program = &module.program;
        let structs = build_struct_layouts(program, module.implicit_pub)?;
        let enums = build_enum_layouts(program, module.implicit_pub)?;
        validate_type_names(program, &structs, &enums)?;
        validate_structs(unit, module.name.as_str(), program, &structs, exports)?;
        validate_enums(program, &enums)?;
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
                    pub_export: function.pub_export || module.implicit_pub,
                },
            );
        }
        let imports = program.imports.iter().map(|i| i.name.clone()).collect();
        Ok(Self {
            module_name: &module.name,
            imports,
            exports,
            functions,
            structs,
            enums,
        })
    }

    fn check_program(&self, program: &Program) -> XResult<()> {
        for function in &program.functions {
            self.check_function(function)?;
        }
        Ok(())
    }

    fn check_function(&self, function: &Function) -> XResult<()> {
        self.validate_signature_type(
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
            self.validate_signature_type(&param.ty, param.ty_span)?;
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
                let binding_ty = if let Some(enum_ref) =
                    annotation.as_ref().and_then(TypeName::enum_ref)
                    && self.is_declared_enum(enum_ref)
                {
                    let span = annotation_span.unwrap_or(*name_span);
                    self.resolve_enum_layout(enum_ref, span)?;
                    if let Expr::Call { callee, .. } = value {
                        if self.is_enum_variant_constructor(enum_ref, value, span)
                            || !self.functions.contains_key(callee)
                        {
                            self.check_enum_constructor(enum_ref, value, span, locals)?;
                        } else {
                            let value_ty = self.check_expr(value, locals)?;
                            if value_ty == TypeName::Void {
                                return Err(Diagnostic::type_error_at(
                                    format!("cannot bind void value to `{name}`"),
                                    value.span(),
                                ));
                            }
                            if annotation.as_ref() != Some(&value_ty) {
                                return Err(Diagnostic::type_error_at(
                                    format!(
                                        "cannot assign {} to {}",
                                        type_name_display(&value_ty),
                                        type_name_display(annotation.as_ref().unwrap())
                                    ),
                                    value.span(),
                                ));
                            }
                        }
                    } else {
                        let value_ty = self.check_expr(value, locals)?;
                        if value_ty == TypeName::Void {
                            return Err(Diagnostic::type_error_at(
                                format!("cannot bind void value to `{name}`"),
                                value.span(),
                            ));
                        }
                        if annotation.as_ref() != Some(&value_ty) {
                            return Err(Diagnostic::type_error_at(
                                format!(
                                    "cannot assign {} to {}",
                                    type_name_display(&value_ty),
                                    type_name_display(annotation.as_ref().unwrap())
                                ),
                                value.span(),
                            ));
                        }
                    }
                    annotation.clone().unwrap()
                } else if let Some(struct_ref) = annotation.as_ref().and_then(TypeName::struct_ref)
                {
                    self.validate_struct_local_type(
                        struct_ref,
                        annotation_span.unwrap_or(*name_span),
                    )?;
                    if matches!(value, Expr::StructLiteral { .. }) {
                        self.check_struct_literal(struct_ref, value, locals)?;
                    } else {
                        let value_ty = self.check_expr(value, locals)?;
                        if value_ty == TypeName::Void {
                            return Err(Diagnostic::type_error_at(
                                format!("cannot bind void value to `{name}`"),
                                value.span(),
                            ));
                        }
                        if annotation.as_ref() != Some(&value_ty) {
                            return Err(Diagnostic::type_error_at(
                                format!(
                                    "cannot assign {} to {}",
                                    type_name_display(&value_ty),
                                    type_name_display(annotation.as_ref().unwrap())
                                ),
                                value.span(),
                            ));
                        }
                    }
                    annotation.clone().unwrap()
                } else {
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
                    annotation.clone().unwrap_or(value_ty)
                };
                if declared.contains(name) {
                    return Err(Diagnostic::type_error_at(
                        format!("duplicate binding `{name}`"),
                        *name_span,
                    ));
                }
                declared.insert(name.clone());
                locals.insert(name.clone(), (binding_ty, *mutable));
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
                if self.is_enum_local_type(target_ty) || target_ty.struct_ref().is_some() {
                    return Err(Diagnostic::type_error_at(
                        format!("cannot assign to enum or struct binding `{name}` as a whole"),
                        *name_span,
                    ));
                }
                if target_ty.array_elem_len().is_some() {
                    return Err(Diagnostic::type_error_at(
                        format!("cannot assign to array binding `{name}` as a whole"),
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
            Stmt::AssignField {
                name,
                name_span,
                field,
                field_span,
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
                        format!("cannot assign to field of const binding `{name}`"),
                        *field_span,
                    ));
                }
                if !matches!(target_ty, TypeName::Named(_) | TypeName::Qualified { .. }) {
                    return Err(Diagnostic::type_error_at(
                        format!(
                            "cannot access field on value of type {}",
                            type_name_display(target_ty)
                        ),
                        *field_span,
                    ));
                };
                let struct_ref = target_ty.struct_ref().expect("struct ref");
                let (_, field_ty) = self.resolve_struct_field(struct_ref, field, *field_span)?;
                let value_ty = self.check_expr(value, locals)?;
                if value_ty != *field_ty {
                    return Err(Diagnostic::type_error_at(
                        format!(
                            "field assignment type mismatch: expected {}, got {}",
                            type_name_display(field_ty),
                            type_name_display(&value_ty)
                        ),
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
                    let actual = if expected_return.enum_ref().is_some()
                        || expected_return.struct_ref().is_some()
                    {
                        self.check_expr_for_type(expr, locals, expected_return)?
                    } else {
                        self.check_expr(expr, locals)?
                    };
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
            Expr::QualifiedCall {
                module,
                callee,
                args,
                span,
            } => self.check_qualified_call(module, callee, args, locals, *span),
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
            Expr::StructLiteral { span, .. } => Err(Diagnostic::type_error_at(
                "struct literals are only supported in struct bindings",
                *span,
            )),
            Expr::FieldAccess {
                base,
                field,
                field_span,
                span: _,
            } => {
                let Expr::Variable {
                    name,
                    span: base_span,
                } = base.as_ref()
                else {
                    return Err(Diagnostic::type_error_at(
                        "field access base must be a variable",
                        base.span(),
                    ));
                };
                let Some((base_ty, _)) = locals.get(name) else {
                    return Err(Diagnostic::type_error_at(
                        format!("unknown variable `{name}`"),
                        *base_span,
                    ));
                };
                if !matches!(base_ty, TypeName::Named(_) | TypeName::Qualified { .. }) {
                    return Err(Diagnostic::type_error_at(
                        format!(
                            "cannot access field on value of type {}",
                            type_name_display(base_ty)
                        ),
                        *field_span,
                    ));
                };
                let struct_ref = base_ty.struct_ref().expect("struct ref");
                if self.is_enum_local_type(base_ty) {
                    return Err(Diagnostic::type_error_at(
                        "cannot access field on enum value",
                        *field_span,
                    ));
                }
                let (_, field_ty) = self.resolve_struct_field(struct_ref, field, *field_span)?;
                Ok(field_ty.clone())
            }
            Expr::Match {
                scrutinee,
                arms,
                span,
            } => self.check_match(scrutinee, arms, locals, *span),
        }
    }

    fn check_match(
        &self,
        scrutinee: &Expr,
        arms: &[MatchArm],
        locals: &HashMap<String, (TypeName, bool)>,
        span: Span,
    ) -> XResult<TypeName> {
        let scrutinee_ty = self.check_expr(scrutinee, locals)?;
        let Some(enum_ref) = scrutinee_ty.enum_ref() else {
            return Err(Diagnostic::type_error_at(
                "match scrutinee must be an enum type",
                scrutinee.span(),
            ));
        };
        let layout = self.resolve_enum_layout(enum_ref, scrutinee.span())?;
        let mut covered = HashSet::new();
        let mut has_wildcard = false;
        let mut arm_ty: Option<TypeName> = None;
        for arm in arms {
            match &arm.pattern {
                Pattern::Wildcard { .. } => {
                    if has_wildcard {
                        return Err(Diagnostic::type_error_at(
                            "duplicate match arm",
                            arm.pattern.span(),
                        ));
                    }
                    has_wildcard = true;
                }
                Pattern::Variant { name, binding, .. } => {
                    if !covered.insert(name.clone()) {
                        return Err(Diagnostic::type_error_at(
                            format!("duplicate match arm `{name}`"),
                            arm.pattern.span(),
                        ));
                    }
                    let Some((_, payload)) = layout.variants.iter().find(|(v, _)| v == name) else {
                        return Err(Diagnostic::type_error_at(
                            format!("unknown variant `{name}` in match pattern"),
                            arm.pattern.span(),
                        ));
                    };
                    if payload.is_some() != binding.is_some() {
                        return Err(Diagnostic::type_error_at(
                            format!("pattern `{name}` payload mismatch"),
                            arm.pattern.span(),
                        ));
                    }
                }
            }
            let mut arm_locals = locals.clone();
            if let Pattern::Variant {
                name,
                binding: Some(binding),
                binding_span: Some(binding_span),
                ..
            } = &arm.pattern
            {
                if binding != "_"
                    && let Some((_, Some(payload_ty))) =
                        layout.variants.iter().find(|(v, _)| v == name)
                {
                    arm_locals.insert(binding.clone(), (payload_ty.clone(), false));
                    let _ = binding_span;
                }
            }
            let body_ty = self.check_match_body(&arm.body, &mut arm_locals, span)?;
            if body_ty == TypeName::Void {
                return Err(Diagnostic::type_error_at(
                    "match arm cannot produce void",
                    arm.body.span(),
                ));
            }
            if let Some(expected) = &arm_ty {
                if expected != &body_ty {
                    return Err(Diagnostic::type_error_at(
                        format!(
                            "match arm type mismatch: expected {}, got {}",
                            type_name_display(expected),
                            type_name_display(&body_ty)
                        ),
                        arm.body.span(),
                    ));
                }
            } else {
                arm_ty = Some(body_ty);
            }
        }
        if !has_wildcard {
            for (variant, _) in &layout.variants {
                if !covered.contains(variant) {
                    return Err(Diagnostic::type_error_at(
                        format!("non-exhaustive match: missing variant `{variant}`"),
                        span,
                    ));
                }
            }
        }
        arm_ty.ok_or_else(|| Diagnostic::type_error_at("match must have at least one arm", span))
    }

    fn check_match_body(
        &self,
        body: &MatchBody,
        locals: &mut HashMap<String, (TypeName, bool)>,
        _span: Span,
    ) -> XResult<TypeName> {
        match body {
            MatchBody::Expr(expr) => self.check_expr(expr, locals),
            MatchBody::Block(stmts) => {
                let mut declared = HashSet::new();
                for stmt in stmts {
                    self.check_stmt(stmt, locals, &mut declared, &TypeName::I32, 0)?;
                }
                let Some(Stmt::Expr(expr)) = stmts.last() else {
                    return Err(Diagnostic::type_error_at(
                        "match arm block must end with an expression statement",
                        body.span(),
                    ));
                };
                self.check_expr(expr, locals)
            }
        }
    }

    fn check_enum_constructor(
        &self,
        enum_ref: EnumRef<'_>,
        value: &Expr,
        span: Span,
        locals: &HashMap<String, (TypeName, bool)>,
    ) -> XResult<()> {
        let Expr::Call { callee, args, .. } = value else {
            return Err(Diagnostic::type_error_at(
                "enum local requires variant constructor initializer",
                value.span(),
            ));
        };
        let layout = self.resolve_enum_layout(enum_ref, span)?;
        let Some((_, payload)) = layout.variants.iter().find(|(name, _)| name == callee) else {
            return Err(Diagnostic::type_error_at(
                format!("unknown variant `{callee}`"),
                value.span(),
            ));
        };
        match payload {
            None => {
                if !args.is_empty() {
                    return Err(Diagnostic::type_error_at(
                        format!("constructor `{callee}` expects 0 arguments"),
                        value.span(),
                    ));
                }
            }
            Some(expected_ty) => {
                if args.len() != 1 {
                    return Err(Diagnostic::type_error_at(
                        format!("constructor `{callee}` expects 1 argument"),
                        value.span(),
                    ));
                }
                let actual = self.check_expr(&args[0], locals)?;
                if &actual != expected_ty {
                    return Err(Diagnostic::type_error_at(
                        format!(
                            "constructor argument type mismatch: expected {}, got {}",
                            type_name_display(expected_ty),
                            type_name_display(&actual)
                        ),
                        args[0].span(),
                    ));
                }
            }
        }
        Ok(())
    }

    fn check_expr_for_type(
        &self,
        expr: &Expr,
        locals: &HashMap<String, (TypeName, bool)>,
        expected: &TypeName,
    ) -> XResult<TypeName> {
        if let Some(enum_ref) = expected.enum_ref()
            && self.is_known_enum(enum_ref, expr.span())
            && let Expr::Call { callee, .. } = expr
            && (self.is_enum_variant_constructor(enum_ref, expr, expr.span())
                || !self.functions.contains_key(callee))
        {
            self.check_enum_constructor(enum_ref, expr, expr.span(), locals)?;
            return Ok(expected.clone());
        }
        if let Some(struct_ref) = expected.struct_ref()
            && self.resolve_struct_layout(struct_ref, expr.span()).is_ok()
            && matches!(expr, Expr::StructLiteral { .. })
        {
            self.check_struct_literal(struct_ref, expr, locals)?;
            return Ok(expected.clone());
        }
        let actual = self.check_expr(expr, locals)?;
        if &actual != expected {
            return Err(Diagnostic::type_error_at(
                format!(
                    "expected {}, got {}",
                    type_name_display(expected),
                    type_name_display(&actual)
                ),
                expr.span(),
            ));
        }
        Ok(actual)
    }

    fn is_known_enum(&self, enum_ref: EnumRef<'_>, span: Span) -> bool {
        self.resolve_enum_layout(enum_ref, span).is_ok()
    }

    fn is_declared_enum(&self, enum_ref: EnumRef<'_>) -> bool {
        match enum_ref {
            EnumRef::Local(name) => self.enums.contains_key(name),
            EnumRef::Qualified { module, name } => self
                .exports
                .get(module)
                .is_some_and(|module_exports| module_exports.enums.contains_key(name)),
        }
    }

    fn is_enum_local_type(&self, ty: &TypeName) -> bool {
        ty.enum_ref()
            .and_then(|r| self.resolve_enum_layout(r, Span::point(1, 1)).ok())
            .is_some()
    }

    fn resolve_enum_layout(&self, enum_ref: EnumRef<'_>, span: Span) -> XResult<&EnumLayout> {
        match enum_ref {
            EnumRef::Local(name) => self.enums.get(name).ok_or_else(|| {
                Diagnostic::type_error_at(format!("unknown enum type `{name}`"), span)
            }),
            EnumRef::Qualified { module, name } => {
                if module != self.module_name && !self.imports.contains(module) {
                    return Err(Diagnostic::type_error_at(
                        format!("unknown module `{module}`"),
                        span,
                    ));
                }
                let module_exports = self.exports.get(module).ok_or_else(|| {
                    Diagnostic::type_error_at(format!("unknown module `{module}`"), span)
                })?;
                let layout = module_exports.enums.get(name).ok_or_else(|| {
                    Diagnostic::type_error_at(format!("unknown enum type `{module}.{name}`"), span)
                })?;
                if module != self.module_name && !layout.pub_export {
                    return Err(Diagnostic::type_error_at(
                        format!("cannot use private enum `{name}` from module `{module}`"),
                        span,
                    ));
                }
                Ok(layout)
            }
        }
    }

    fn check_struct_literal(
        &self,
        struct_ref: StructRef<'_>,
        value: &Expr,
        locals: &HashMap<String, (TypeName, bool)>,
    ) -> XResult<()> {
        let Expr::StructLiteral { elements, span } = value else {
            return Err(Diagnostic::type_error_at(
                "expected struct literal",
                value.span(),
            ));
        };
        let layout = self.resolve_struct_layout(struct_ref, *span)?;
        if elements.len() != layout.fields.len() {
            return Err(Diagnostic::type_error_at(
                format!(
                    "struct literal length mismatch: expected {} fields, got {}",
                    layout.fields.len(),
                    elements.len()
                ),
                *span,
            ));
        }
        for (index, (element, (_, expected_ty))) in
            elements.iter().zip(layout.fields.iter()).enumerate()
        {
            let actual_ty = self.check_expr(element, locals)?;
            if actual_ty != *expected_ty {
                return Err(Diagnostic::type_error_at(
                    format!(
                        "struct field {index} type mismatch: expected {}, got {}",
                        type_name_display(expected_ty),
                        type_name_display(&actual_ty)
                    ),
                    element.span(),
                ));
            }
        }
        Ok(())
    }

    fn resolve_struct_field<'b>(
        &'b self,
        struct_ref: StructRef<'_>,
        field: &str,
        span: Span,
    ) -> XResult<(&'b str, &'b TypeName)> {
        let layout = self.resolve_struct_layout(struct_ref, span)?;
        layout
            .fields
            .iter()
            .find(|(name, _)| name == field)
            .map(|(name, ty)| (name.as_str(), ty))
            .ok_or_else(|| {
                Diagnostic::type_error_at(
                    format!(
                        "struct `{}` has no field `{field}`",
                        struct_ref_display(struct_ref)
                    ),
                    span,
                )
            })
    }

    fn resolve_struct_layout<'b>(
        &'b self,
        struct_ref: StructRef<'_>,
        span: Span,
    ) -> XResult<&'b StructLayout> {
        match struct_ref {
            StructRef::Local(name) => self.structs.get(name).ok_or_else(|| {
                Diagnostic::type_error_at(format!("unknown struct type `{name}`"), span)
            }),
            StructRef::Qualified { module, name } => {
                if module != self.module_name && !self.imports.contains(module) {
                    return Err(Diagnostic::type_error_at(
                        format!("unknown module `{module}`"),
                        span,
                    ));
                }
                let module_exports = self.exports.get(module).ok_or_else(|| {
                    Diagnostic::type_error_at(format!("unknown module `{module}`"), span)
                })?;
                let layout = module_exports.structs.get(name).ok_or_else(|| {
                    Diagnostic::type_error_at(
                        format!("unknown struct type `{module}.{name}`"),
                        span,
                    )
                })?;
                if module != self.module_name && !layout.pub_export {
                    return Err(Diagnostic::type_error_at(
                        format!("cannot use private struct `{name}` from module `{module}`"),
                        span,
                    ));
                }
                Ok(layout)
            }
        }
    }

    fn validate_struct_local_type(&self, struct_ref: StructRef<'_>, span: Span) -> XResult<()> {
        self.resolve_struct_layout(struct_ref, span).map(|_| ())
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
        Ok(self.qualify_cross_module_type(self.module_name, &sig.return_type))
    }

    fn check_qualified_call(
        &self,
        module: &str,
        callee: &str,
        args: &[Expr],
        locals: &HashMap<String, (TypeName, bool)>,
        span: Span,
    ) -> XResult<TypeName> {
        if !self.imports.contains(module) {
            return Err(Diagnostic::type_error_at(
                format!("unknown module `{module}`"),
                span,
            ));
        }
        let Some(module_exports) = self.exports.get(module) else {
            return Err(Diagnostic::type_error_at(
                format!("unknown module `{module}`"),
                span,
            ));
        };
        let Some(sig) = module_exports.functions.get(callee) else {
            return Err(Diagnostic::type_error_at(
                format!("unknown function `{module}.{callee}`"),
                span,
            ));
        };
        if module != self.module_name && !sig.pub_export {
            return Err(Diagnostic::type_error_at(
                format!("`{callee}` is private to module `{module}`"),
                span,
            ));
        }
        for expected in sig.params.iter().chain(std::iter::once(&sig.return_type)) {
            self.validate_cross_module_signature_named_type(module, expected, span)?;
        }
        if sig.params.len() != args.len() {
            return Err(Diagnostic::type_error_at(
                format!(
                    "function `{module}.{callee}` expects {} arguments, got {}",
                    sig.params.len(),
                    args.len()
                ),
                span,
            ));
        }
        for (arg, expected) in args.iter().zip(sig.params.iter()) {
            let expected = self.qualify_cross_module_type(module, expected);
            let actual = self.check_expr(arg, locals)?;
            if actual == TypeName::Void {
                return Err(Diagnostic::type_error_at(
                    "cannot pass void expression as an argument",
                    arg.span(),
                ));
            }
            if actual != expected {
                return Err(Diagnostic::type_error_at(
                    format!(
                        "argument type mismatch: expected {}, got {}",
                        type_name_display(&expected),
                        type_name_display(&actual)
                    ),
                    arg.span(),
                ));
            }
        }
        Ok(self.qualify_cross_module_type(module, &sig.return_type))
    }

    fn validate_cross_module_signature_named_type(
        &self,
        owner_module: &str,
        ty: &TypeName,
        span: Span,
    ) -> XResult<()> {
        let (type_module, type_name) = match ty {
            TypeName::Named(name) => (owner_module, name.as_str()),
            TypeName::Qualified { module, name } => (module.as_str(), name.as_str()),
            _ => return Ok(()),
        };
        if type_module == self.module_name {
            return Ok(());
        }
        let module_exports = self.exports.get(type_module).ok_or_else(|| {
            Diagnostic::type_error_at(format!("unknown module `{type_module}`"), span)
        })?;
        if let Some(layout) = module_exports.enums.get(type_name) {
            if !layout.pub_export {
                return Err(Diagnostic::type_error_at(
                    format!("cannot use private enum `{type_name}` from module `{type_module}`"),
                    span,
                ));
            }
        } else if let Some(layout) = module_exports.structs.get(type_name)
            && !layout.pub_export
        {
            return Err(Diagnostic::type_error_at(
                format!("cannot use private struct `{type_name}` from module `{type_module}`"),
                span,
            ));
        }
        Ok(())
    }

    fn qualify_cross_module_type(&self, owner_module: &str, ty: &TypeName) -> TypeName {
        if owner_module == self.module_name {
            return ty.clone();
        }
        match ty {
            TypeName::Named(name) => {
                if self.exports.get(owner_module).is_some_and(|exports| {
                    exports.enums.contains_key(name) || exports.structs.contains_key(name)
                }) {
                    TypeName::Qualified {
                        module: owner_module.to_owned(),
                        name: name.clone(),
                    }
                } else {
                    ty.clone()
                }
            }
            _ => ty.clone(),
        }
    }

    fn is_enum_variant_constructor(&self, enum_ref: EnumRef<'_>, value: &Expr, span: Span) -> bool {
        let Expr::Call { callee, .. } = value else {
            return false;
        };
        self.resolve_enum_layout(enum_ref, span)
            .ok()
            .is_some_and(|layout| layout.variants.iter().any(|(name, _)| name == callee))
    }

    fn validate_signature_type(&self, ty: &TypeName, span: Span) -> XResult<()> {
        match ty {
            TypeName::I32 | TypeName::Bool | TypeName::Str | TypeName::Void => Ok(()),
            TypeName::Array { .. } => Err(Diagnostic::type_error_at(
                "array type not supported in function signatures yet",
                span,
            )),
            TypeName::Named(name) => {
                if self.is_known_enum(EnumRef::Local(name), span) {
                    Ok(())
                } else if self.structs.contains_key(name) {
                    Ok(())
                } else {
                    Err(Diagnostic::type_error_at(
                        format!("unknown type `{name}`"),
                        span,
                    ))
                }
            }
            TypeName::Qualified { module, name } => {
                if module != self.module_name && !self.imports.contains(module) {
                    return Err(Diagnostic::type_error_at(
                        format!("unknown module `{module}`"),
                        span,
                    ));
                }
                let Some(module_exports) = self.exports.get(module) else {
                    return Err(Diagnostic::type_error_at(
                        format!("unknown module `{module}`"),
                        span,
                    ));
                };
                if self
                    .exports
                    .get(module)
                    .is_some_and(|exports| exports.enums.contains_key(name))
                {
                    self.resolve_enum_layout(EnumRef::Qualified { module, name }, span)?;
                    Ok(())
                } else if module_exports.structs.contains_key(name) {
                    self.resolve_struct_layout(StructRef::Qualified { module, name }, span)?;
                    Ok(())
                } else {
                    Err(Diagnostic::type_error_at(
                        format!("unknown type `{module}.{name}`"),
                        span,
                    ))
                }
            }
        }
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
        | Stmt::AssignField { .. }
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
        Stmt::AssignField { field_span, .. } => *field_span,
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
        TypeName::Named(name) | TypeName::Qualified { name, .. } => Err(Diagnostic::type_error_at(
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
        TypeName::I32 => "i32".to_owned(),
        TypeName::Bool => "bool".to_owned(),
        TypeName::Str => "str".to_owned(),
        TypeName::Void => "void".to_owned(),
        TypeName::Named(name) => name.clone(),
        TypeName::Qualified { module, name } => format!("{module}.{name}"),
        TypeName::Array { elem, len } => format!("{}[{len}]", type_name_display(elem)),
    }
}

fn struct_ref_display(struct_ref: StructRef<'_>) -> String {
    match struct_ref {
        StructRef::Local(name) => name.to_owned(),
        StructRef::Qualified { module, name } => format!("{module}.{name}"),
    }
}

fn build_enum_layouts(
    program: &Program,
    implicit_pub: bool,
) -> XResult<HashMap<String, EnumLayout>> {
    let mut enums = HashMap::new();
    for enum_decl in &program.enums {
        let variants = enum_decl
            .variants
            .iter()
            .map(|variant| {
                (
                    variant.name.clone(),
                    variant.payload.as_ref().map(|p| p.ty.clone()),
                )
            })
            .collect();
        enums.insert(
            enum_decl.name.clone(),
            EnumLayout {
                variants,
                pub_export: enum_decl.pub_export || implicit_pub,
            },
        );
    }
    Ok(enums)
}

fn validate_type_names(
    program: &Program,
    structs: &HashMap<String, StructLayout>,
    enums: &HashMap<String, EnumLayout>,
) -> XResult<()> {
    for struct_decl in &program.structs {
        if enums.contains_key(&struct_decl.name) {
            return Err(Diagnostic::type_error_at(
                format!("duplicate type name `{}`", struct_decl.name),
                struct_decl.name_span,
            ));
        }
    }
    for enum_decl in &program.enums {
        if structs.contains_key(&enum_decl.name) {
            return Err(Diagnostic::type_error_at(
                format!("duplicate type name `{}`", enum_decl.name),
                enum_decl.name_span,
            ));
        }
    }
    Ok(())
}

fn validate_enums(program: &Program, layouts: &HashMap<String, EnumLayout>) -> XResult<()> {
    let mut enum_names = HashSet::new();
    for enum_decl in &program.enums {
        if !enum_names.insert(enum_decl.name.clone()) {
            return Err(Diagnostic::type_error_at(
                format!("duplicate enum `{}`", enum_decl.name),
                enum_decl.name_span,
            ));
        }
        if enum_decl.variants.is_empty() {
            return Err(Diagnostic::type_error_at(
                format!("enum `{}` must have at least one variant", enum_decl.name),
                enum_decl.name_span,
            ));
        }
        let mut variant_names = HashSet::new();
        for variant in &enum_decl.variants {
            if !variant_names.insert(variant.name.clone()) {
                return Err(Diagnostic::type_error_at(
                    format!(
                        "duplicate variant `{}` in enum `{}`",
                        variant.name, enum_decl.name
                    ),
                    variant.name_span,
                ));
            }
            if let Some(payload) = &variant.payload {
                validate_enum_payload_type(&enum_decl.name, &payload.ty, payload.ty_span)?;
            }
        }
        let _ = layouts;
    }
    Ok(())
}

fn validate_enum_payload_type(enum_name: &str, ty: &TypeName, span: Span) -> XResult<()> {
    match ty {
        TypeName::I32 | TypeName::Bool => Ok(()),
        TypeName::Str => Err(Diagnostic::type_error_at(
            format!("payload type `str` is not supported in enum `{enum_name}` yet"),
            span,
        )),
        TypeName::Void => Err(Diagnostic::type_error_at(
            format!("payload type cannot be void in enum `{enum_name}`"),
            span,
        )),
        TypeName::Named(name) => Err(Diagnostic::type_error_at(
            format!("unknown payload type `{name}` in enum `{enum_name}`"),
            span,
        )),
        TypeName::Qualified { module, name } => Err(Diagnostic::type_error_at(
            format!("qualified payload type `{module}.{name}` is not supported yet"),
            span,
        )),
        TypeName::Array { .. } => Err(Diagnostic::type_error_at(
            format!("array payload types are not supported in enum `{enum_name}` yet"),
            span,
        )),
    }
}

fn build_unit_exports(unit: &CompilationUnit) -> XResult<HashMap<String, ModuleExports>> {
    let mut exports = HashMap::new();
    for module in unit.modules.values() {
        let structs = build_struct_layouts(&module.program, module.implicit_pub)?;
        let enums = build_enum_layouts(&module.program, module.implicit_pub)?;
        let mut functions = HashMap::new();
        for function in &module.program.functions {
            functions.insert(
                function.name.clone(),
                FunctionSig {
                    params: function.params.iter().map(|p| p.ty.clone()).collect(),
                    first_param_span: function.params.first().map(|p| p.name_span),
                    return_type: function.return_type.clone(),
                    return_type_span: function.return_type_span,
                    name_span: function.name_span,
                    pub_export: function.pub_export || module.implicit_pub,
                },
            );
        }
        exports.insert(
            module.name.clone(),
            ModuleExports {
                functions,
                structs,
                enums,
            },
        );
    }
    Ok(exports)
}

fn validate_main_across_unit(
    unit: &CompilationUnit,
    exports: &HashMap<String, ModuleExports>,
) -> XResult<()> {
    let mut main_modules = Vec::new();
    for (name, module_exports) in exports {
        if module_exports.functions.contains_key("main") {
            main_modules.push(name.clone());
        }
    }
    if main_modules.is_empty() {
        return Err(Diagnostic::type_error("program must define `main()`", 1, 1));
    }
    if main_modules.len() > 1 {
        return Err(Diagnostic::type_error(
            format!(
                "multiple `main` functions found in modules: {}",
                main_modules.join(", ")
            ),
            1,
            1,
        ));
    }
    if main_modules[0] != unit.entry {
        return Err(Diagnostic::type_error(
            format!("`main` must be defined in entry module `{}`", unit.entry),
            1,
            1,
        ));
    }
    let entry_exports = exports.get(&unit.entry).expect("entry exports");
    validate_main_signature(&entry_exports.functions)
}

fn build_struct_layouts(
    program: &Program,
    implicit_pub: bool,
) -> XResult<HashMap<String, StructLayout>> {
    let mut structs = HashMap::new();
    for struct_decl in &program.structs {
        let fields = struct_decl
            .fields
            .iter()
            .map(|field| (field.name.clone(), field.ty.clone()))
            .collect();
        structs.insert(
            struct_decl.name.clone(),
            StructLayout {
                fields,
                pub_export: struct_decl.pub_export || implicit_pub,
            },
        );
    }
    Ok(structs)
}

fn validate_structs(
    unit: &CompilationUnit,
    module_name: &str,
    program: &Program,
    layouts: &HashMap<String, StructLayout>,
    exports: &HashMap<String, ModuleExports>,
) -> XResult<()> {
    let mut struct_names = HashSet::new();
    for struct_decl in &program.structs {
        if !struct_names.insert(struct_decl.name.clone()) {
            return Err(Diagnostic::type_error_at(
                format!("duplicate struct `{}`", struct_decl.name),
                struct_decl.name_span,
            ));
        }

        if struct_decl.fields.is_empty() {
            return Err(Diagnostic::type_error_at(
                format!("struct `{}` must have at least one field", struct_decl.name),
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
            validate_struct_field_type(
                unit,
                module_name,
                &struct_decl.name,
                &field.ty,
                field.ty_span,
                layouts,
                exports,
            )?;
        }
    }
    Ok(())
}

fn validate_struct_field_type(
    unit: &CompilationUnit,
    module_name: &str,
    struct_name: &str,
    ty: &TypeName,
    span: Span,
    layouts: &HashMap<String, StructLayout>,
    exports: &HashMap<String, ModuleExports>,
) -> XResult<()> {
    match ty {
        TypeName::I32 | TypeName::Bool => Ok(()),
        TypeName::Str => Err(Diagnostic::type_error_at(
            format!("field type `str` is not supported in struct `{struct_name}` yet"),
            span,
        )),
        TypeName::Void => Err(Diagnostic::type_error_at(
            format!("field type cannot be void in struct `{struct_name}`"),
            span,
        )),
        TypeName::Named(name) => {
            if layouts.contains_key(name) {
                Err(Diagnostic::type_error_at(
                    format!("nested struct field type `{name}` is not supported yet"),
                    span,
                ))
            } else {
                Err(Diagnostic::type_error_at(
                    format!("unknown type `{name}` in struct `{struct_name}`"),
                    span,
                ))
            }
        }
        TypeName::Qualified { module, name } => {
            let module_exports = exports.get(module).ok_or_else(|| {
                Diagnostic::type_error_at(format!("unknown module `{module}`"), span)
            })?;
            let layout = module_exports.structs.get(name).ok_or_else(|| {
                Diagnostic::type_error_at(format!("unknown struct type `{module}.{name}`"), span)
            })?;
            if module != module_name && !layout.pub_export {
                return Err(Diagnostic::type_error_at(
                    format!("cannot use private struct `{name}` from module `{module}`"),
                    span,
                ));
            }
            if module != module_name
                && !unit
                    .modules
                    .get(module_name)
                    .map(|loaded| {
                        loaded
                            .program
                            .imports
                            .iter()
                            .any(|import| import.name == *module)
                    })
                    .unwrap_or(false)
            {
                return Err(Diagnostic::type_error_at(
                    format!("unknown module `{module}`"),
                    span,
                ));
            }
            Err(Diagnostic::type_error_at(
                format!("nested struct field type `{module}.{name}` is not supported yet"),
                span,
            ))
        }
        TypeName::Array { .. } => Err(Diagnostic::type_error_at(
            format!("array field types are not supported in struct `{struct_name}` yet"),
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
