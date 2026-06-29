use std::collections::HashMap;

use crate::ast::{Expr, Field, Function, Program, Stmt, StructDecl, TypeName};
use crate::diagnostic::{Diagnostic, Span};
use crate::{lexer, parser, typeck};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct SymbolId(pub u32);

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SymbolKind {
    Module,
    Import,
    Function,
    Parameter,
    Variable { mutable: bool },
    Struct,
    StructField,
    TypeName,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Symbol {
    pub id: SymbolId,
    pub name: String,
    pub kind: SymbolKind,
    pub span: Span,
    pub ty: Option<TypeName>,
    pub parent: Option<SymbolId>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReferenceKind {
    Read,
    Write,
    Call,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Reference {
    pub span: Span,
    pub symbol_id: SymbolId,
    pub kind: ReferenceKind,
}

#[derive(Debug, Clone, Default)]
pub struct SemanticIndex {
    pub symbols: Vec<Symbol>,
    pub references: Vec<Reference>,
}

impl SemanticIndex {
    pub fn symbol_at_offset(&self, offset: usize) -> Option<SymbolId> {
        self.symbols
            .iter()
            .find(|symbol| symbol.span.start_byte <= offset && offset < symbol.span.end_byte)
            .map(|symbol| symbol.id)
    }

    pub fn reference_at_offset(&self, offset: usize) -> Option<&Reference> {
        self.references.iter().find(|reference| {
            reference.span.start_byte <= offset && offset < reference.span.end_byte
        })
    }

    pub fn symbol(&self, id: SymbolId) -> Option<&Symbol> {
        self.symbols.get(id.0 as usize)
    }

    pub fn references_to(&self, id: SymbolId) -> Vec<&Reference> {
        self.references
            .iter()
            .filter(|reference| reference.symbol_id == id)
            .collect()
    }

    pub fn completion_symbols(&self) -> &[Symbol] {
        &self.symbols
    }
}

#[derive(Debug, Clone)]
pub struct AnalysisResult {
    pub program: Option<Program>,
    pub diagnostics: Vec<Diagnostic>,
    pub index: SemanticIndex,
}

pub fn analyze_source(source: &str) -> AnalysisResult {
    let mut diagnostics = Vec::new();

    let tokens = match lexer::lex(source) {
        Ok(tokens) => tokens,
        Err(err) => {
            diagnostics.push(err);
            return AnalysisResult {
                program: None,
                diagnostics,
                index: SemanticIndex::default(),
            };
        }
    };

    let program = match parser::parse(tokens) {
        Ok(program) => program,
        Err(err) => {
            diagnostics.push(err);
            return AnalysisResult {
                program: None,
                diagnostics,
                index: SemanticIndex::default(),
            };
        }
    };

    let index = build_index(&program);

    if let Err(err) = typeck::check(&program) {
        diagnostics.push(err);
    }

    AnalysisResult {
        program: Some(program),
        diagnostics,
        index,
    }
}

pub fn format_type_name(ty: &TypeName) -> String {
    match ty {
        TypeName::I32 => "i32".to_owned(),
        TypeName::Bool => "bool".to_owned(),
        TypeName::Str => "str".to_owned(),
        TypeName::Void => "void".to_owned(),
        TypeName::Named(name) => name.clone(),
        TypeName::Qualified { module, name } => format!("{module}.{name}"),
        TypeName::Array { elem, len } => format!("{}[{len}]", format_type_name(elem)),
    }
}

fn build_index(program: &Program) -> SemanticIndex {
    let mut index = SemanticIndex::default();
    let mut next_id = 0u32;

    let mut alloc_id = || {
        let id = SymbolId(next_id);
        next_id += 1;
        id
    };

    if let Some(module) = &program.module {
        index.symbols.push(Symbol {
            id: alloc_id(),
            name: module.clone(),
            kind: SymbolKind::Module,
            span: Span::point(1, 1),
            ty: None,
            parent: None,
        });
    }

    for import in &program.imports {
        index.symbols.push(Symbol {
            id: alloc_id(),
            name: import.name.clone(),
            kind: SymbolKind::Import,
            span: Span::point(1, 1),
            ty: None,
            parent: None,
        });
    }

    for struct_decl in &program.structs {
        index_struct(&mut index, struct_decl, &mut alloc_id);
    }

    for function in &program.functions {
        index_function(&mut index, function, &mut alloc_id);
    }

    index
}

fn index_struct(
    index: &mut SemanticIndex,
    struct_decl: &StructDecl,
    alloc_id: &mut impl FnMut() -> SymbolId,
) {
    let struct_id = alloc_id();
    index.symbols.push(Symbol {
        id: struct_id,
        name: struct_decl.name.clone(),
        kind: SymbolKind::Struct,
        span: struct_decl.name_span,
        ty: None,
        parent: None,
    });

    for field in &struct_decl.fields {
        index.symbols.push(Symbol {
            id: alloc_id(),
            name: field.name.clone(),
            kind: SymbolKind::StructField,
            span: field.name_span,
            ty: Some(field.ty.clone()),
            parent: Some(struct_id),
        });
        push_type_symbol(index, &field.ty, field.ty_span, alloc_id);
    }
}

fn index_function(
    index: &mut SemanticIndex,
    function: &Function,
    alloc_id: &mut impl FnMut() -> SymbolId,
) {
    let function_id = alloc_id();
    index.symbols.push(Symbol {
        id: function_id,
        name: function.name.clone(),
        kind: SymbolKind::Function,
        span: function.name_span,
        ty: Some(function.return_type.clone()),
        parent: None,
    });

    if let Some(span) = function.return_type_span {
        push_type_symbol(index, &function.return_type, span, alloc_id);
    }

    let mut scope = HashMap::new();

    for param in &function.params {
        let param_id = alloc_id();
        index.symbols.push(Symbol {
            id: param_id,
            name: param.name.clone(),
            kind: SymbolKind::Parameter,
            span: param.name_span,
            ty: Some(param.ty.clone()),
            parent: Some(function_id),
        });
        push_type_symbol(index, &param.ty, param.ty_span, alloc_id);
        scope.insert(param.name.clone(), param_id);
    }

    index_block(index, &function.body, function_id, &mut scope, alloc_id);
}

fn index_block(
    index: &mut SemanticIndex,
    stmts: &[Stmt],
    function_id: SymbolId,
    scope: &mut HashMap<String, SymbolId>,
    alloc_id: &mut impl FnMut() -> SymbolId,
) {
    for stmt in stmts {
        match stmt {
            Stmt::Let {
                mutable,
                name,
                name_span,
                annotation,
                annotation_span,
                value,
            } => {
                if let Some(ty) = annotation
                    && let Some(span) = annotation_span
                {
                    push_type_symbol(index, ty, *span, alloc_id);
                }
                let local_id = alloc_id();
                index.symbols.push(Symbol {
                    id: local_id,
                    name: name.clone(),
                    kind: SymbolKind::Variable { mutable: *mutable },
                    span: *name_span,
                    ty: annotation.clone(),
                    parent: Some(function_id),
                });
                scope.insert(name.clone(), local_id);
                index_expr(index, value, scope, alloc_id);
            }
            Stmt::Assign {
                name,
                name_span,
                value,
            } => {
                if let Some(symbol_id) = scope.get(name) {
                    index.references.push(Reference {
                        span: *name_span,
                        symbol_id: *symbol_id,
                        kind: ReferenceKind::Read,
                    });
                }
                index_expr(index, value, scope, alloc_id);
            }
            Stmt::Return {
                value: Some(expr), ..
            } => {
                index_expr(index, expr, scope, alloc_id);
            }
            Stmt::Return { value: None, .. } => {}
            Stmt::Expr(expr) => index_expr(index, expr, scope, alloc_id),
            Stmt::If {
                condition,
                then_body,
                else_body,
                ..
            } => {
                index_expr(index, condition, scope, alloc_id);
                index_block(index, then_body, function_id, scope, alloc_id);
                index_block(index, else_body, function_id, scope, alloc_id);
            }
            Stmt::While {
                condition, body, ..
            } => {
                index_expr(index, condition, scope, alloc_id);
                index_block(index, body, function_id, scope, alloc_id);
            }
            Stmt::Break { .. } | Stmt::Continue { .. } => {}
            Stmt::AssignIndex {
                name,
                name_span,
                index: idx,
                value,
            } => {
                if let Some(symbol_id) = scope.get(name) {
                    index.references.push(Reference {
                        span: *name_span,
                        symbol_id: *symbol_id,
                        kind: ReferenceKind::Read,
                    });
                }
                index_expr(index, idx, scope, alloc_id);
                index_expr(index, value, scope, alloc_id);
            }
            Stmt::AssignField {
                name,
                name_span,
                field,
                field_span,
                value,
            } => {
                if let Some(symbol_id) = scope.get(name) {
                    index.references.push(Reference {
                        span: *name_span,
                        symbol_id: *symbol_id,
                        kind: ReferenceKind::Read,
                    });
                }
                if let Some(local_ty) = resolve_local_type_from_scope(index, scope, name)
                    && let TypeName::Named(struct_name) = local_ty
                    && let Some(field_id) = find_struct_field_symbol(index, &struct_name, field)
                {
                    index.references.push(Reference {
                        span: *field_span,
                        symbol_id: field_id,
                        kind: ReferenceKind::Write,
                    });
                }
                index_expr(index, value, scope, alloc_id);
            }
        }
    }
}

fn index_expr(
    index: &mut SemanticIndex,
    expr: &Expr,
    scope: &HashMap<String, SymbolId>,
    alloc_id: &mut impl FnMut() -> SymbolId,
) {
    let _ = alloc_id;
    match expr {
        Expr::Integer { .. } | Expr::Bool { .. } => {}
        Expr::String { span, .. } => {
            let _ = span;
        }
        Expr::Variable { name, span } => {
            if let Some(symbol_id) = scope.get(name) {
                index.references.push(Reference {
                    span: *span,
                    symbol_id: *symbol_id,
                    kind: ReferenceKind::Read,
                });
            }
        }
        Expr::Call { callee, args, span } => {
            if let Some(function_id) = find_function(index, callee) {
                index.references.push(Reference {
                    span: *span,
                    symbol_id: function_id,
                    kind: ReferenceKind::Call,
                });
            }
            for arg in args {
                index_expr(index, arg, scope, alloc_id);
            }
        }
        Expr::QualifiedCall { args, .. } => {
            for arg in args {
                index_expr(index, arg, scope, alloc_id);
            }
        }
        Expr::Unary { expr, .. } => index_expr(index, expr, scope, alloc_id),
        Expr::Binary { left, right, .. } => {
            index_expr(index, left, scope, alloc_id);
            index_expr(index, right, scope, alloc_id);
        }
        Expr::ArrayLiteral { elements, .. } => {
            for element in elements {
                index_expr(index, element, scope, alloc_id);
            }
        }
        Expr::Index {
            base, index: idx, ..
        } => {
            index_expr(index, base, scope, alloc_id);
            index_expr(index, idx, scope, alloc_id);
        }
        Expr::StructLiteral { elements, .. } => {
            for element in elements {
                index_expr(index, element, scope, alloc_id);
            }
        }
        Expr::FieldAccess {
            base,
            field,
            field_span,
            ..
        } => index_field_reference(index, scope, base, field, *field_span, alloc_id),
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MemberCompletionItem {
    pub name: String,
    pub ty: TypeName,
    pub replace_start: usize,
    pub replace_end: usize,
}

pub fn member_field_completions(
    analysis: &AnalysisResult,
    source: &str,
    offset: usize,
) -> Option<Vec<MemberCompletionItem>> {
    let (base_name, prefix, replace_start, replace_end) = member_access_context(source, offset)?;
    let local_ty = resolve_local_type_at_offset(&analysis.index, &base_name, offset)?;
    let TypeName::Named(struct_name) = local_ty else {
        return None;
    };
    let struct_decl = analysis
        .program
        .as_ref()?
        .structs
        .iter()
        .find(|decl| decl.name == struct_name)?;

    let items = struct_decl
        .fields
        .iter()
        .filter(|field| field.name.starts_with(&prefix))
        .map(|field| MemberCompletionItem {
            name: field.name.clone(),
            ty: field.ty.clone(),
            replace_start,
            replace_end,
        })
        .collect();
    Some(items)
}

fn member_access_context(source: &str, offset: usize) -> Option<(String, String, usize, usize)> {
    if source.is_empty() {
        return None;
    }
    let index = offset.min(source.len());
    let before = &source[..index];
    let dot = before.rfind('.')?;
    let after_dot = &before[dot + 1..];
    if !after_dot.is_empty()
        && !after_dot
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || ch == '_')
    {
        return None;
    }
    let base_end = dot;
    let mut base_start = base_end;
    while base_start > 0 && is_identifier_char(source.as_bytes()[base_start - 1]) {
        base_start -= 1;
    }
    let base_name = source.get(base_start..base_end)?.to_owned();
    if base_name.is_empty() {
        return None;
    }
    Some((base_name, after_dot.to_owned(), dot + 1, index))
}

fn resolve_local_type_at_offset(
    index: &SemanticIndex,
    name: &str,
    offset: usize,
) -> Option<TypeName> {
    index
        .symbols
        .iter()
        .filter(|symbol| {
            matches!(
                symbol.kind,
                SymbolKind::Variable { .. } | SymbolKind::Parameter
            ) && symbol.name == name
                && symbol.span.start_byte <= offset
        })
        .max_by_key(|symbol| symbol.span.start_byte)
        .and_then(|symbol| symbol.ty.clone())
}

fn find_struct_field_symbol(
    index: &SemanticIndex,
    struct_name: &str,
    field: &str,
) -> Option<SymbolId> {
    let struct_id = index.symbols.iter().find_map(|symbol| {
        if symbol.kind == SymbolKind::Struct && symbol.name == struct_name {
            Some(symbol.id)
        } else {
            None
        }
    })?;
    index.symbols.iter().find_map(|symbol| {
        if symbol.kind == SymbolKind::StructField
            && symbol.parent == Some(struct_id)
            && symbol.name == field
        {
            Some(symbol.id)
        } else {
            None
        }
    })
}

fn index_field_reference(
    index: &mut SemanticIndex,
    scope: &HashMap<String, SymbolId>,
    base: &Expr,
    field: &str,
    field_span: Span,
    alloc_id: &mut impl FnMut() -> SymbolId,
) {
    index_expr(index, base, scope, alloc_id);
    let Expr::Variable { name, .. } = base else {
        return;
    };
    let Some(local_ty) = resolve_local_type_from_scope(index, scope, name) else {
        return;
    };
    let TypeName::Named(struct_name) = local_ty else {
        return;
    };
    if let Some(field_id) = find_struct_field_symbol(index, &struct_name, field) {
        index.references.push(Reference {
            span: field_span,
            symbol_id: field_id,
            kind: ReferenceKind::Read,
        });
    }
}

fn resolve_local_type_from_scope(
    index: &SemanticIndex,
    scope: &HashMap<String, SymbolId>,
    name: &str,
) -> Option<TypeName> {
    scope
        .get(name)
        .and_then(|id| index.symbol(*id))
        .and_then(|symbol| symbol.ty.clone())
}

fn find_function(index: &SemanticIndex, name: &str) -> Option<SymbolId> {
    index.symbols.iter().find_map(|symbol| {
        if symbol.kind == SymbolKind::Function && symbol.name == name {
            Some(symbol.id)
        } else {
            None
        }
    })
}

fn push_type_symbol(
    index: &mut SemanticIndex,
    ty: &TypeName,
    span: Span,
    alloc_id: &mut impl FnMut() -> SymbolId,
) {
    if matches!(ty, TypeName::Named(_) | TypeName::Qualified { .. }) {
        return;
    }
    index.symbols.push(Symbol {
        id: alloc_id(),
        name: format_type_name(ty),
        kind: SymbolKind::TypeName,
        span,
        ty: Some(ty.clone()),
        parent: None,
    });
}

/// Context for building rich hover documentation at a byte offset.
pub struct HoverContext<'a> {
    pub source: &'a str,
    pub program: Option<&'a Program>,
    pub index: &'a SemanticIndex,
    pub offset: usize,
}

pub fn build_hover_at_offset(ctx: &HoverContext<'_>) -> Option<String> {
    if let Some(symbol_id) = ctx.index.symbol_at_offset(ctx.offset) {
        let symbol = ctx.index.symbol(symbol_id)?;
        return Some(render_symbol_hover(ctx, symbol, None));
    }

    if let Some(reference) = ctx.index.reference_at_offset(ctx.offset) {
        let symbol = ctx.index.symbol(reference.symbol_id)?;
        return Some(render_symbol_hover(ctx, symbol, Some(reference)));
    }

    word_at_offset(ctx.source, ctx.offset).map(|word| {
        if let Some(program) = ctx.program
            && let Some(struct_decl) = find_struct_decl(program, word)
        {
            return render_struct_type_hover(struct_decl);
        }
        render_keyword_or_type_hover(word)
    })
}

fn render_symbol_hover(
    ctx: &HoverContext<'_>,
    symbol: &Symbol,
    reference: Option<&Reference>,
) -> String {
    let mut sections = Vec::new();

    match &symbol.kind {
        SymbolKind::Function => {
            sections.push(format!("**Function** `{}`", symbol.name));
            if let Some(function) = ctx
                .program
                .and_then(|program| find_function_decl(program, &symbol.name))
            {
                sections.push(signature_block(&format_function_signature(function)));
                sections.push(String::from(
                    "User-defined function. Parameters are mutable locals in the function body.",
                ));
                if !function.params.is_empty() {
                    sections.push(render_parameter_list(&function.params));
                }
                sections.push(format!(
                    "**Return** — `{}`",
                    format_type_name(&function.return_type)
                ));
            } else {
                let ret = symbol
                    .ty
                    .as_ref()
                    .map(format_type_name)
                    .unwrap_or_else(|| "void".to_owned());
                sections.push(signature_block(&format!("{ret} {}()", symbol.name)));
            }
        }
        SymbolKind::Parameter => {
            sections.push(format!("**Parameter** `{}`", symbol.name));
            let ty = symbol
                .ty
                .as_ref()
                .map(format_type_name)
                .unwrap_or_else(|| "?".to_owned());
            sections.push(signature_block(&format!("{ty} {}", symbol.name)));
            if let Some(parent) = symbol.parent.and_then(|id| ctx.index.symbol(id)) {
                sections.push(format!(
                    "Parameter of function `{}`. Assignable inside the function body.",
                    parent.name
                ));
            }
            sections.push(format!("**Type** — `{ty}`"));
        }
        SymbolKind::Variable { mutable } => {
            let label = if *mutable {
                "Local variable"
            } else {
                "Local constant"
            };
            sections.push(format!("**{label}** `{}`", symbol.name));
            let ty = symbol
                .ty
                .as_ref()
                .map(format_type_name)
                .unwrap_or_else(|| "?".to_owned());
            let decl = if *mutable {
                format!("{ty} {}", symbol.name)
            } else {
                format!("const {ty} {}", symbol.name)
            };
            sections.push(signature_block(&decl));
            sections.push(if *mutable {
                String::from("Mutable local binding. Type must match on assignment.")
            } else {
                String::from("Immutable local binding declared with `const`. Cannot be reassigned.")
            });
            sections.push(format!("**Type** — `{ty}`"));
            sections.push(format!(
                "**Mutability** — {}",
                if *mutable {
                    "mutable"
                } else {
                    "immutable (`const`)"
                }
            ));
            if let (Some(TypeName::Named(struct_name)), Some(program)) =
                (symbol.ty.as_ref(), ctx.program)
                && let Some(struct_decl) = find_struct_decl(program, struct_name)
            {
                sections.push(render_field_list(&struct_decl.fields));
                sections.push(String::from(
                    "**Fields** — read with `.field`; assign with `.field = expr` on mutable bindings.",
                ));
            }
        }
        SymbolKind::Struct => {
            sections.push(format!("**Struct** `{}`", symbol.name));
            if let Some(struct_decl) = ctx
                .program
                .and_then(|program| find_struct_decl(program, &symbol.name))
            {
                sections.push(signature_block(&format_struct_decl(struct_decl)));
                if !struct_decl.fields.is_empty() {
                    sections.push(render_field_list(&struct_decl.fields));
                }
            } else {
                sections.push(signature_block(&format!("struct {} {{ … }}", symbol.name)));
            }
            sections.push(String::from(
                "**Status** — struct locals, literals, and field access are supported in v0.3. Struct-typed function signatures are not supported yet.",
            ));
        }
        SymbolKind::StructField => {
            sections.push(format!("**Struct field** `{}`", symbol.name));
            let ty = symbol
                .ty
                .as_ref()
                .map(format_type_name)
                .unwrap_or_else(|| "?".to_owned());
            sections.push(signature_block(&format!("{ty} {};", symbol.name)));
            if let Some(parent_id) = symbol.parent
                && let Some(parent) = ctx.index.symbol(parent_id)
            {
                sections.push(format!("Field of struct `{}`.", parent.name));
            }
            sections.push(format!("**Type** — `{ty}`"));
            sections.push(String::from(
                "**Usage** — `binding.field` to read; `binding.field = expr;` to write (mutable binding).",
            ));
        }
        SymbolKind::TypeName => {
            if let Some(ty) = &symbol.ty {
                sections.extend(render_type_hover(&format_type_name(ty), ty));
            } else {
                sections.push(format!("**Type** `{}`", symbol.name));
                sections.push(signature_block(&symbol.name));
            }
        }
        SymbolKind::Module => {
            sections.push(format!("**Module** `{}`", symbol.name));
            sections.push(signature_block(&format!("module {}", symbol.name)));
            sections.push(String::from(
                "Optional file-level module name. No semantic effect in v0.1 — not validated against the file path or other modules.",
            ));
        }
        SymbolKind::Import => {
            sections.push(format!("**Import** `{}`", symbol.name));
            sections.push(signature_block(&format!("import {}", symbol.name)));
            sections.push(String::from(
                "Import declaration. No semantic effect in v0.1 — symbols are not resolved across files.",
            ));
        }
    }

    if let Some(reference) = reference {
        sections.push(render_reference_note(reference));
    }

    sections.push(render_footer(ctx, symbol));
    sections.join("\n\n")
}

fn render_keyword_or_type_hover(word: &str) -> String {
    if let Some(ty) = parse_builtin_type(word) {
        return render_type_hover(word, &ty).join("\n\n");
    }

    let mut sections = Vec::new();
    sections.push(format!("**Keyword** `{word}`"));
    if let Some(description) = keyword_documentation(word) {
        sections.push(description.to_owned());
    } else {
        sections.push(format!("XLang keyword `{word}`."));
    }
    sections.join("\n\n")
}

fn render_struct_type_hover(struct_decl: &StructDecl) -> String {
    let mut sections = Vec::new();
    sections.push(format!("**Struct type** `{}`", struct_decl.name));
    sections.push(signature_block(&format_struct_decl(struct_decl)));
    if !struct_decl.fields.is_empty() {
        sections.push(render_field_list(&struct_decl.fields));
    }
    sections.push(String::from(
        "**Usage** — `TypeName name = { … };` with a positional struct literal.",
    ));
    sections.push(String::from("**Codegen** — supported (scalar fields only)"));
    sections.join("\n\n")
}

fn render_type_hover(name: &str, ty: &TypeName) -> Vec<String> {
    let mut sections = Vec::new();
    sections.push(format!("**Primitive type** `{name}`"));
    sections.push(signature_block(name));
    if let Some(summary) = type_summary(ty) {
        sections.push(summary.to_owned());
    }
    if let Some(usage) = type_usage(ty) {
        sections.push(usage.to_owned());
    }
    if let Some(codegen) = type_codegen_note(ty) {
        sections.push(codegen.to_owned());
    }
    sections
}

fn render_parameter_list(params: &[crate::ast::Param]) -> String {
    let items = params
        .iter()
        .map(|param| format!("- `{}` — `{}`", param.name, format_type_name(&param.ty)))
        .collect::<Vec<_>>()
        .join("\n");
    format!("**Parameters**\n{items}")
}

fn render_field_list(fields: &[Field]) -> String {
    let items = fields
        .iter()
        .map(|field| format!("- `{}` — `{}`", field.name, format_type_name(&field.ty)))
        .collect::<Vec<_>>()
        .join("\n");
    format!("**Fields**\n{items}")
}

fn render_reference_note(reference: &Reference) -> String {
    let action = match reference.kind {
        ReferenceKind::Call => "Function call",
        ReferenceKind::Read => "Read reference",
        ReferenceKind::Write => "Write reference",
    };
    format!("**Usage** — {action}")
}

fn render_footer(ctx: &HoverContext<'_>, symbol: &Symbol) -> String {
    let refs = ctx.index.references_to(symbol.id).len();
    let line = symbol.span.start_line;
    let ref_label = match refs {
        0 => "no references",
        1 => "1 reference",
        _ => return format!("*Line {line} · {refs} references*"),
    };
    format!("*Line {line} · {ref_label}*")
}

fn signature_block(code: &str) -> String {
    format!("```xlang\n{code}\n```")
}

fn format_function_signature(function: &Function) -> String {
    let params = function
        .params
        .iter()
        .map(|param| format!("{} {}", format_type_name(&param.ty), param.name))
        .collect::<Vec<_>>()
        .join(", ");
    format!(
        "{} {}({})",
        format_type_name(&function.return_type),
        function.name,
        params
    )
}

fn format_struct_decl(struct_decl: &StructDecl) -> String {
    let fields = struct_decl
        .fields
        .iter()
        .map(|field| format!("    {} {};", format_type_name(&field.ty), field.name))
        .collect::<Vec<_>>()
        .join("\n");
    if fields.is_empty() {
        format!("struct {} {{}}", struct_decl.name)
    } else {
        format!("struct {} {{\n{fields}\n}}", struct_decl.name)
    }
}

fn find_function_decl<'a>(program: &'a Program, name: &str) -> Option<&'a Function> {
    program
        .functions
        .iter()
        .find(|function| function.name == name)
}

fn find_struct_decl<'a>(program: &'a Program, name: &str) -> Option<&'a StructDecl> {
    program
        .structs
        .iter()
        .find(|struct_decl| struct_decl.name == name)
}

fn word_at_offset(source: &str, offset: usize) -> Option<&str> {
    if source.is_empty() {
        return None;
    }
    let index = offset.min(source.len().saturating_sub(1));
    if !is_identifier_char(source.as_bytes()[index]) {
        return None;
    }
    let mut start = index;
    let mut end = index + 1;
    while start > 0 && is_identifier_char(source.as_bytes()[start - 1]) {
        start -= 1;
    }
    while end < source.len() && is_identifier_char(source.as_bytes()[end]) {
        end += 1;
    }
    source.get(start..end).filter(|word| !word.is_empty())
}

fn is_identifier_char(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || byte == b'_'
}

fn parse_builtin_type(word: &str) -> Option<TypeName> {
    match word {
        "i32" => Some(TypeName::I32),
        "bool" => Some(TypeName::Bool),
        "str" => Some(TypeName::Str),
        "void" => Some(TypeName::Void),
        _ => None,
    }
}

fn type_summary(ty: &TypeName) -> Option<&'static str> {
    match ty {
        TypeName::I32 => Some("32-bit signed integer (LLVM `i32`)."),
        TypeName::Bool => Some("Boolean value (`true` or `false`), lowered as LLVM `i1`."),
        TypeName::Str => Some("String type accepted by the frontend; not lowered to LLVM in v0.1."),
        TypeName::Void => Some("Absence of value. Valid only as a function return type."),
        TypeName::Named(_) | TypeName::Qualified { .. } => Some("User-defined struct type with stack-allocated locals (v0.3)."),
        TypeName::Array { elem, .. } => match elem.as_ref() {
            TypeName::I32 => Some("Fixed-size stack array of `i32` elements (v0.2)."),
            TypeName::Bool => Some("Fixed-size stack array of `bool` elements (v0.2)."),
            _ => Some("Fixed-size stack array (frontend-only element type)."),
        },
    }
}

fn type_usage(ty: &TypeName) -> Option<&'static str> {
    match ty {
        TypeName::I32 | TypeName::Bool => {
            Some("**Usage** — expressions, locals, parameters, return types")
        }
        TypeName::Str => {
            Some("**Usage** — expressions, locals, parameters, return types (frontend only)")
        }
        TypeName::Void => Some("**Usage** — function return type only"),
        TypeName::Named(_) | TypeName::Qualified { .. } => Some("**Usage** — local bindings with struct literals"),
        TypeName::Array { .. } => Some("**Usage** — local bindings with array literals"),
    }
}

fn type_codegen_note(ty: &TypeName) -> Option<&'static str> {
    match ty {
        TypeName::I32 | TypeName::Bool | TypeName::Void => Some("**Codegen** — supported"),
        TypeName::Str => {
            Some("**Codegen** — not supported (`x check` may pass, `x emit-llvm` fails)")
        }
        TypeName::Named(_) | TypeName::Qualified { .. } => Some("**Codegen** — supported (scalar fields only)"),
        TypeName::Array { elem, .. } => match elem.as_ref() {
            TypeName::I32 | TypeName::Bool => Some("**Codegen** — supported (with bounds checks)"),
            _ => Some("**Codegen** — not supported"),
        },
    }
}

fn keyword_documentation(keyword: &str) -> Option<&'static str> {
    match keyword {
        "module" => Some(
            "Declares an optional module name at the top of the file. At most once, before imports and items.",
        ),
        "import" => Some(
            "Declares a dependency name. Parsed for syntax only; cross-file resolution is not implemented.",
        ),
        "struct" => Some(
            "Begins a struct declaration: `struct Name { type field; … }`. Use struct types in local bindings with `{ … }` literals (v0.3).",
        ),
        "const" => Some(
            "Marks an immutable local binding: `const type name = expr;`. The binding cannot be reassigned.",
        ),
        "return" => Some(
            "Exits the current function. Use `return expr;` when the function returns a value, or `return;` in `void` functions.",
        ),
        "if" => Some(
            "Conditional execution. Condition must be `bool`. Both branches are blocks: `if cond { … } else { … }`.",
        ),
        "else" => Some("Optional alternative branch attached to an `if` statement."),
        "while" => Some(
            "Repeated execution while the condition is `bool` and true. Supports `break` and `continue`.",
        ),
        "break" => Some("Exits the innermost enclosing `while` loop."),
        "continue" => {
            Some("Jumps to the next condition evaluation of the innermost enclosing `while` loop.")
        }
        "true" | "false" => Some("Boolean literal of type `bool`."),
        _ => None,
    }
}

/// Legacy helper kept for simple symbol-only hovers.
pub fn hover_markdown(symbol: &Symbol) -> String {
    render_symbol_hover(
        &HoverContext {
            source: "",
            program: None,
            index: &SemanticIndex::default(),
            offset: 0,
        },
        symbol,
        None,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    const DEMO: &str = r#"
module main

i32 add(i32 a, i32 b) {
    return a + b;
}

i32 main() {
    i32 x = add(40, 2);
    return x;
}
"#;

    #[test]
    fn builds_symbol_index_for_demo_program() {
        let result = analyze_source(DEMO);
        assert!(result.diagnostics.is_empty());
        assert!(result.index.symbols.iter().any(|s| s.name == "add"));
        assert!(result.index.symbols.iter().any(|s| s.name == "main"));
        assert!(!result.index.references.is_empty());
    }

    #[test]
    fn hover_function_shows_full_signature_and_parameters() {
        let result = analyze_source(DEMO);
        let offset = DEMO.find("add").unwrap();
        let hover = build_hover_at_offset(&HoverContext {
            source: DEMO,
            program: result.program.as_ref(),
            index: &result.index,
            offset,
        })
        .expect("hover text");
        assert!(hover.contains("**Function** `add`"));
        assert!(hover.contains("i32 add(i32 a, i32 b)"));
        assert!(hover.contains("**Parameters**"));
        assert!(hover.contains("`a` — `i32`"));
    }

    #[test]
    fn hover_keyword_documents_return() {
        let result = analyze_source(DEMO);
        let offset = DEMO.find("return").unwrap();
        let hover = build_hover_at_offset(&HoverContext {
            source: DEMO,
            program: result.program.as_ref(),
            index: &result.index,
            offset,
        })
        .expect("hover text");
        assert!(hover.contains("**Keyword** `return`"));
        assert!(hover.contains("void"));
    }

    #[test]
    fn hover_type_documents_i32() {
        let result = analyze_source(DEMO);
        let symbol = result
            .index
            .symbols
            .iter()
            .find(|symbol| symbol.name == "i32" && symbol.kind == SymbolKind::TypeName)
            .expect("i32 symbol");
        let hover = build_hover_at_offset(&HoverContext {
            source: DEMO,
            program: result.program.as_ref(),
            index: &result.index,
            offset: symbol.span.start_byte,
        })
        .expect("hover text");
        assert!(hover.contains("**Primitive type** `i32`"));
        assert!(hover.contains("**Codegen** — supported"));
    }

    const STRUCT_DEMO: &str = r#"
struct Vec2 {
    i32 x;
    i32 y;
}

i32 main() {
    Vec2 p = { 3, 4 };
    return p.x + p.y;
}
"#;

    #[test]
    fn hover_struct_field_shows_type_and_parent() {
        let result = analyze_source(STRUCT_DEMO);
        assert!(result.diagnostics.is_empty());
        let offset = STRUCT_DEMO.find(".x").unwrap() + 1;
        let hover = build_hover_at_offset(&HoverContext {
            source: STRUCT_DEMO,
            program: result.program.as_ref(),
            index: &result.index,
            offset,
        })
        .expect("hover text");
        assert!(hover.contains("**Struct field** `x`"));
        assert!(hover.contains("Field of struct `Vec2`"));
    }

    #[test]
    fn hover_struct_type_name_in_binding() {
        let result = analyze_source(STRUCT_DEMO);
        let offset = STRUCT_DEMO.find("Vec2 p").unwrap();
        let hover = build_hover_at_offset(&HoverContext {
            source: STRUCT_DEMO,
            program: result.program.as_ref(),
            index: &result.index,
            offset,
        })
        .expect("hover text");
        assert!(hover.contains("**Struct type** `Vec2`"));
    }

    #[test]
    fn member_field_completions_after_dot() {
        let result = analyze_source(STRUCT_DEMO);
        let offset = STRUCT_DEMO.find("p.x").unwrap() + 2;
        let items = member_field_completions(&result, STRUCT_DEMO, offset).expect("completions");
        let names: Vec<_> = items.iter().map(|item| item.name.as_str()).collect();
        assert!(names.contains(&"x"));
        assert!(names.contains(&"y"));
    }

    #[test]
    fn indexes_field_reference_for_goto_definition() {
        let result = analyze_source(STRUCT_DEMO);
        let offset = STRUCT_DEMO.find(".y").unwrap() + 1;
        let reference = result
            .index
            .reference_at_offset(offset)
            .expect("field reference");
        let symbol = result.index.symbol(reference.symbol_id).expect("symbol");
        assert_eq!(symbol.kind, SymbolKind::StructField);
        assert_eq!(symbol.name, "y");
    }
}
