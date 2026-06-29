use std::collections::HashMap;

use inkwell::IntPredicate;
use inkwell::basic_block::BasicBlock;
use inkwell::builder::Builder;
use inkwell::context::Context;
use inkwell::module::Module;
use inkwell::targets::TargetTriple;
use inkwell::types::{BasicMetadataTypeEnum, BasicType, BasicTypeEnum, StructType};
use inkwell::values::{
    BasicMetadataValueEnum, BasicValue, BasicValueEnum, FunctionValue, PointerValue,
};

use crate::ast::{
    BinaryOp, EnumRef, Expr, Function, MatchArm, MatchBody, Pattern, Program, Stmt, StructRef,
    TypeName, UnaryOp,
};
use crate::diagnostic::{Diagnostic, XResult};
use crate::modules::CompilationUnit;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct LlvmOptions {
    pub target_triple: Option<String>,
}

pub fn emit_compilation_unit(
    unit: &CompilationUnit,
    options: &LlvmOptions,
) -> XResult<std::collections::HashMap<String, String>> {
    let mut output = std::collections::HashMap::new();
    for module_name in unit.modules.keys() {
        let ir = emit_module_ir(unit, module_name, options)?;
        output.insert(module_name.clone(), ir);
    }
    Ok(output)
}

pub fn emit_llvm_ir(program: &Program) -> XResult<String> {
    emit_llvm_ir_with_options(program, &LlvmOptions::default())
}

pub fn emit_llvm_ir_with_options(program: &Program, options: &LlvmOptions) -> XResult<String> {
    let unit = CompilationUnit::from_program(program.clone())?;
    let mut modules = emit_compilation_unit(&unit, options)?;
    let entry = unit.entry.clone();
    modules.remove(&entry).ok_or_else(|| {
        Diagnostic::backend(format!("missing LLVM IR for entry module `{entry}`"), 1, 1)
    })
}

fn emit_module_ir(
    unit: &CompilationUnit,
    module_name: &str,
    options: &LlvmOptions,
) -> XResult<String> {
    let context = Context::create();
    let is_entry = module_name == unit.entry;
    let mut emitter = LlvmEmitter::new(
        &context,
        unit,
        module_name,
        is_entry,
        options.target_triple.as_deref(),
    );
    emitter.emit_program()
}

fn mangle_function(module_name: &str, function_name: &str, is_entry_main: bool) -> String {
    if function_name == "main" && is_entry_main {
        "main".to_owned()
    } else {
        format!("xlang.{module_name}.{function_name}")
    }
}

fn struct_ir_name(module_name: &str, struct_name: &str) -> String {
    format!("{module_name}.{struct_name}")
}

fn enum_ir_name(module_name: &str, enum_name: &str) -> String {
    format!("{module_name}.{enum_name}.tagged")
}

fn enum_key_from_type(module_name: &str, ty: &TypeName) -> Option<String> {
    match ty.enum_ref()? {
        EnumRef::Local(name) => Some(enum_ir_name(module_name, name)),
        EnumRef::Qualified { module, name } => Some(enum_ir_name(module, name)),
    }
}

fn enum_key_from_ref(module_name: &str, enum_ref: EnumRef<'_>) -> String {
    match enum_ref {
        EnumRef::Local(name) => enum_ir_name(module_name, name),
        EnumRef::Qualified { module, name } => enum_ir_name(module, name),
    }
}

fn struct_key_from_type(module_name: &str, ty: &TypeName) -> Option<String> {
    match ty.struct_ref()? {
        StructRef::Local(name) => Some(struct_ir_name(module_name, name)),
        StructRef::Qualified { module, name } => Some(struct_ir_name(module, name)),
    }
}

struct LlvmEmitter<'ctx, 'unit> {
    context: &'ctx Context,
    module: Module<'ctx>,
    builder: Builder<'ctx>,
    unit: &'unit CompilationUnit,
    module_name: &'unit str,
    is_entry: bool,
    program: &'unit Program,
    functions: HashMap<String, FunctionValue<'ctx>>,
    struct_types: HashMap<String, StructType<'ctx>>,
    enum_types: HashMap<String, StructType<'ctx>>,
}

struct FunctionEmitter<'emit, 'ctx, 'unit> {
    context: &'ctx Context,
    builder: &'emit Builder<'ctx>,
    function: FunctionValue<'ctx>,
    functions: &'emit HashMap<String, FunctionValue<'ctx>>,
    module: &'emit Module<'ctx>,
    unit: &'unit CompilationUnit,
    module_name: &'unit str,
    struct_types: &'emit HashMap<String, StructType<'ctx>>,
    enum_types: &'emit HashMap<String, StructType<'ctx>>,
    locals: HashMap<String, Local<'ctx>>,
    function_return_type: TypeName,
    loop_stack: Vec<LoopLabels<'ctx>>,
    terminated: bool,
}

struct LoopLabels<'ctx> {
    cond: BasicBlock<'ctx>,
    end: BasicBlock<'ctx>,
}

#[derive(Clone)]
struct Local<'ctx> {
    pointer: PointerValue<'ctx>,
    ty: TypeName,
    scalar_ty: Option<BasicTypeEnum<'ctx>>,
}

impl<'ctx, 'unit> LlvmEmitter<'ctx, 'unit> {
    fn new(
        context: &'ctx Context,
        unit: &'unit CompilationUnit,
        module_name: &'unit str,
        is_entry: bool,
        target_triple: Option<&str>,
    ) -> Self {
        let module = context.create_module(&format!("xlang.{module_name}"));
        if let Some(triple) = target_triple {
            module.set_triple(&TargetTriple::create(triple));
        } else if let Some(triple) = host_target_triple() {
            module.set_triple(&TargetTriple::create(triple));
        }
        let program = &unit.modules.get(module_name).expect("module").program;
        Self {
            context,
            module,
            builder: context.create_builder(),
            unit,
            module_name,
            is_entry,
            program,
            functions: HashMap::new(),
            struct_types: HashMap::new(),
            enum_types: HashMap::new(),
        }
    }

    fn emit_program(&mut self) -> XResult<String> {
        self.ensure_supported()?;
        self.declare_struct_types()?;
        self.declare_enum_types()?;
        self.declare_functions()?;
        self.declare_external_functions()?;
        for function in &self.program.functions {
            self.emit_function(function)?;
        }
        self.module.verify().map_err(|message| {
            Diagnostic::backend(format!("LLVM verifier failed: {message}"), 1, 1)
        })?;
        Ok(self.module.print_to_string().to_string())
    }

    fn declare_external_functions(&mut self) -> XResult<()> {
        let mut needed = std::collections::HashSet::new();
        for function in &self.program.functions {
            collect_qualified_calls(&function.body, &mut needed);
        }
        for (import_module, callee) in needed {
            if import_module == self.module_name {
                continue;
            }
            let Some(target) = self.unit.modules.get(&import_module) else {
                continue;
            };
            let Some(target_fn) = target.program.functions.iter().find(|f| f.name == callee) else {
                continue;
            };
            ensure_declared_enum_types_for_signature(
                self.context,
                self.unit,
                &import_module,
                &mut self.enum_types,
                &target_fn.return_type,
                &target_fn
                    .params
                    .iter()
                    .map(|param| param.ty.clone())
                    .collect::<Vec<_>>(),
            )?;
            ensure_declared_struct_types_for_signature(
                self.context,
                self.unit,
                &import_module,
                &mut self.struct_types,
                &target_fn.return_type,
                &target_fn
                    .params
                    .iter()
                    .map(|param| param.ty.clone())
                    .collect::<Vec<_>>(),
            )?;
            let symbol = mangle_function(&import_module, &callee, false);
            if self.functions.contains_key(&symbol) {
                continue;
            }
            let params = target_fn
                .params
                .iter()
                .map(|param| {
                    llvm_basic_type(
                        self.context,
                        &self.struct_types,
                        &self.enum_types,
                        self.unit,
                        &import_module,
                        &param.ty,
                    )
                    .map(Into::into)
                })
                .collect::<XResult<Vec<BasicMetadataTypeEnum<'ctx>>>>()?;
            let function_type = match target_fn.return_type {
                TypeName::Void => self.context.void_type().fn_type(&params, false),
                _ => llvm_basic_type(
                    self.context,
                    &self.struct_types,
                    &self.enum_types,
                    self.unit,
                    &import_module,
                    &target_fn.return_type,
                )?
                .fn_type(&params, false),
            };
            let function_value = self.module.add_function(&symbol, function_type, None);
            self.functions.insert(symbol, function_value);
        }
        Ok(())
    }

    fn ensure_supported(&self) -> XResult<()> {
        for struct_decl in &self.program.structs {
            for field in &struct_decl.fields {
                ensure_struct_field_backend_type(&field.ty, field.ty_span)?;
            }
        }
        for enum_decl in &self.program.enums {
            for variant in &enum_decl.variants {
                if let Some(payload) = &variant.payload {
                    ensure_enum_payload_backend_type(&payload.ty, payload.ty_span)?;
                }
            }
        }
        for function in &self.program.functions {
            ensure_backend_signature_type(
                self.unit,
                self.module_name,
                &function.return_type,
                function.return_type_span.unwrap_or(function.name_span),
            )?;
            for param in &function.params {
                ensure_backend_signature_type(
                    self.unit,
                    self.module_name,
                    &param.ty,
                    param.ty_span,
                )?;
            }
            for stmt in &function.body {
                ensure_stmt_supported(self.unit, self.module_name, stmt)?;
            }
        }
        Ok(())
    }

    fn declare_struct_types(&mut self) -> XResult<()> {
        let mut struct_keys = std::collections::HashSet::new();
        for struct_decl in &self.program.structs {
            struct_keys.insert(struct_ir_name(self.module_name, &struct_decl.name));
        }
        collect_referenced_struct_types(
            self.unit,
            self.program,
            self.module_name,
            &mut struct_keys,
        );
        collect_struct_types_in_signatures(
            self.unit,
            self.program,
            self.module_name,
            &mut struct_keys,
        );
        for key in struct_keys {
            let (owner_module, struct_name) = key
                .split_once('.')
                .ok_or_else(|| Diagnostic::backend(format!("invalid struct key `{key}`"), 1, 1))?;
            let owner = self.unit.modules.get(owner_module).ok_or_else(|| {
                Diagnostic::backend(format!("unknown module `{owner_module}`"), 1, 1)
            })?;
            let struct_decl = owner
                .program
                .structs
                .iter()
                .find(|decl| decl.name == struct_name)
                .ok_or_else(|| Diagnostic::backend(format!("unknown struct `{key}`"), 1, 1))?;
            let field_types = struct_decl
                .fields
                .iter()
                .map(|field| llvm_scalar_type(self.context, &field.ty, field.ty_span))
                .collect::<XResult<Vec<BasicTypeEnum<'ctx>>>>()?;
            let struct_type = self
                .context
                .get_struct_type(&key)
                .unwrap_or_else(|| self.context.opaque_struct_type(&key));
            struct_type.set_body(&field_types, false);
            self.struct_types.insert(key, struct_type);
        }
        Ok(())
    }

    fn declare_enum_types(&mut self) -> XResult<()> {
        let mut enum_keys = std::collections::HashSet::new();
        for enum_decl in &self.program.enums {
            enum_keys.insert(enum_ir_name(self.module_name, &enum_decl.name));
        }
        collect_referenced_enum_types(self.unit, self.program, self.module_name, &mut enum_keys);
        collect_enum_types_in_signatures(self.unit, self.program, self.module_name, &mut enum_keys);
        let i32 = self.context.i32_type();
        for key in enum_keys {
            let enum_type = self
                .context
                .get_struct_type(&key)
                .unwrap_or_else(|| self.context.opaque_struct_type(&key));
            enum_type.set_body(&[i32.into(), i32.into()], false);
            self.enum_types.insert(key, enum_type);
        }
        Ok(())
    }

    fn declare_functions(&mut self) -> XResult<()> {
        for function in &self.program.functions {
            let params = function
                .params
                .iter()
                .map(|param| {
                    llvm_basic_type(
                        self.context,
                        &self.struct_types,
                        &self.enum_types,
                        self.unit,
                        self.module_name,
                        &param.ty,
                    )
                    .map(Into::into)
                })
                .collect::<XResult<Vec<BasicMetadataTypeEnum<'ctx>>>>()?;
            let function_type = match function.return_type {
                TypeName::Void => self.context.void_type().fn_type(&params, false),
                _ => llvm_basic_type(
                    self.context,
                    &self.struct_types,
                    &self.enum_types,
                    self.unit,
                    self.module_name,
                    &function.return_type,
                )?
                .fn_type(&params, false),
            };
            let symbol = mangle_function(self.module_name, &function.name, self.is_entry);
            let function_value = self.module.add_function(&symbol, function_type, None);
            self.functions.insert(symbol, function_value);
        }
        Ok(())
    }

    fn emit_function(&self, function: &Function) -> XResult<()> {
        let symbol = mangle_function(self.module_name, &function.name, self.is_entry);
        let function_value = self
            .functions
            .get(&symbol)
            .copied()
            .ok_or_else(|| Diagnostic::backend(format!("unknown function `{symbol}`"), 1, 1))?;
        let entry = self.context.append_basic_block(function_value, "entry");
        self.builder.position_at_end(entry);

        let mut emitter = FunctionEmitter {
            context: self.context,
            builder: &self.builder,
            function: function_value,
            functions: &self.functions,
            module: &self.module,
            unit: self.unit,
            module_name: self.module_name,
            struct_types: &self.struct_types,
            enum_types: &self.enum_types,
            locals: HashMap::new(),
            function_return_type: function.return_type.clone(),
            loop_stack: Vec::new(),
            terminated: false,
        };

        for (index, param) in function.params.iter().enumerate() {
            let value = function_value
                .get_nth_param(index as u32)
                .ok_or_else(|| Diagnostic::backend("missing LLVM function parameter", 1, 1))?;
            value.set_name(&param.name);
            if let Some(enum_key) = enum_key_from_type(self.module_name, &param.ty)
                && self.enum_types.contains_key(&enum_key)
            {
                let enum_ty = self.enum_types.get(&enum_key).copied().expect("enum type");
                let slot = build_value(
                    self.builder
                        .build_alloca(enum_ty, &format!("{}.addr", param.name)),
                )?;
                build_unit(self.builder.build_store(slot, value))?;
                emitter.locals.insert(
                    param.name.clone(),
                    Local {
                        pointer: slot,
                        ty: param.ty.clone(),
                        scalar_ty: None,
                    },
                );
            } else if let Some(struct_key) = struct_key_from_type(self.module_name, &param.ty)
                && self.struct_types.contains_key(&struct_key)
            {
                let struct_ty = self
                    .struct_types
                    .get(&struct_key)
                    .copied()
                    .expect("struct type");
                let slot = build_value(
                    self.builder
                        .build_alloca(struct_ty, &format!("{}.addr", param.name)),
                )?;
                build_unit(self.builder.build_store(slot, value))?;
                emitter.locals.insert(
                    param.name.clone(),
                    Local {
                        pointer: slot,
                        ty: param.ty.clone(),
                        scalar_ty: None,
                    },
                );
            } else {
                let ty = value.get_type();
                let slot = build_value(
                    self.builder
                        .build_alloca(ty, &format!("{}.addr", param.name)),
                )?;
                build_unit(self.builder.build_store(slot, value))?;
                emitter.locals.insert(
                    param.name.clone(),
                    Local {
                        pointer: slot,
                        ty: param.ty.clone(),
                        scalar_ty: Some(ty),
                    },
                );
            }
        }

        for stmt in &function.body {
            emitter.emit_stmt(stmt)?;
        }
        if function.return_type == TypeName::Void && !emitter.terminated {
            build_unit(self.builder.build_return(None))?;
        }
        Ok(())
    }
}

fn host_target_triple() -> Option<&'static str> {
    #[cfg(all(target_arch = "x86_64", target_os = "windows", target_env = "msvc"))]
    const TRIPLE: Option<&str> = Some("x86_64-pc-windows-msvc");

    #[cfg(all(target_arch = "x86_64", target_os = "linux"))]
    const TRIPLE: Option<&str> = Some("x86_64-pc-linux-gnu");

    #[cfg(all(target_arch = "aarch64", target_os = "macos"))]
    const TRIPLE: Option<&str> = Some("arm64-apple-macosx");

    #[cfg(all(target_arch = "x86_64", target_os = "macos"))]
    const TRIPLE: Option<&str> = Some("x86_64-apple-macosx");

    #[cfg(not(any(
        all(target_arch = "x86_64", target_os = "windows", target_env = "msvc"),
        all(target_arch = "x86_64", target_os = "linux"),
        all(target_arch = "aarch64", target_os = "macos"),
        all(target_arch = "x86_64", target_os = "macos"),
    )))]
    const TRIPLE: Option<&str> = None;

    TRIPLE
}

impl<'emit, 'ctx, 'unit> FunctionEmitter<'emit, 'ctx, 'unit> {
    fn emit_stmt(&mut self, stmt: &Stmt) -> XResult<()> {
        if self.terminated {
            return Ok(());
        }
        match stmt {
            Stmt::Let {
                name,
                value,
                annotation,
                ..
            } => {
                let source_span = value.span();
                if let Some(array_ty) = annotation {
                    if let Some((elem, len)) = array_ty.array_elem_len() {
                        self.emit_array_binding(name, elem, len, value, source_span)?;
                        return Ok(());
                    }
                }
                if let Some(enum_ref) = annotation.as_ref().and_then(TypeName::enum_ref) {
                    let ir_key = enum_key_from_ref(self.module_name, enum_ref);
                    if self.enum_types.contains_key(&ir_key) {
                        if self.is_enum_constructor_value(value, &ir_key) {
                            self.emit_enum_binding(
                                name,
                                &ir_key,
                                annotation.as_ref().unwrap(),
                                value,
                                source_span,
                            )?;
                        } else {
                            self.emit_enum_from_expr(
                                name,
                                &ir_key,
                                annotation.as_ref().unwrap(),
                                value,
                                source_span,
                            )?;
                        }
                        return Ok(());
                    }
                }
                if let Some(struct_ref) = annotation.as_ref().and_then(TypeName::struct_ref) {
                    let ir_key = struct_key_from_ref(self.module_name, struct_ref);
                    if self.struct_types.contains_key(&ir_key) {
                        if matches!(value, Expr::StructLiteral { .. }) {
                            self.emit_struct_binding(
                                name,
                                &ir_key,
                                annotation.as_ref().unwrap(),
                                value,
                                source_span,
                            )?;
                        } else {
                            self.emit_struct_from_expr(
                                name,
                                &ir_key,
                                annotation.as_ref().unwrap(),
                                value,
                                source_span,
                            )?;
                        }
                        return Ok(());
                    }
                }
                {
                    let value = self.emit_expr(value)?;
                    let Some(value) = value else {
                        return Err(Diagnostic::backend_at(
                            "cannot bind void value",
                            source_span,
                        ));
                    };
                    let ty = value.get_type();
                    let slot = build_value(self.builder.build_alloca(ty, name.as_str()))?;
                    build_unit(self.builder.build_store(slot, value))?;
                    self.locals.insert(
                        name.clone(),
                        Local {
                            pointer: slot,
                            ty: annotation.clone().unwrap_or(TypeName::I32),
                            scalar_ty: Some(ty),
                        },
                    );
                }
            }
            Stmt::Assign { name, value, .. } => {
                let source_span = value.span();
                let value = self.emit_expr(value)?;
                let Some(value) = value else {
                    return Err(Diagnostic::backend_at(
                        "cannot assign void value",
                        source_span,
                    ));
                };
                let local = self.locals.get(name).ok_or_else(|| {
                    Diagnostic::backend(format!("unknown variable `{name}`"), 1, 1)
                })?;
                if local.ty.array_elem_len().is_some() {
                    return Err(Diagnostic::backend_at(
                        "cannot assign to array binding as a whole",
                        source_span,
                    ));
                }
                if matches!(local.ty, TypeName::Named(_) | TypeName::Qualified { .. }) {
                    return Err(Diagnostic::backend_at(
                        "cannot assign to struct binding as a whole",
                        source_span,
                    ));
                }
                build_unit(self.builder.build_store(local.pointer, value))?;
            }
            Stmt::AssignField {
                name, field, value, ..
            } => {
                let (struct_ptr, struct_key) = {
                    let local = self.locals.get(name).ok_or_else(|| {
                        Diagnostic::backend(format!("unknown variable `{name}`"), 1, 1)
                    })?;
                    if local.ty.struct_ref().is_none() {
                        return Err(Diagnostic::backend_at(
                            "field assignment requires struct binding",
                            value.span(),
                        ));
                    }
                    let struct_key =
                        struct_key_from_type(self.module_name, &local.ty).expect("struct key");
                    (local.pointer, struct_key)
                };
                let field_index = struct_field_index_for_unit(self.unit, &struct_key, field)?;
                let field_ty = struct_field_type_for_unit(self.unit, &struct_key, field_index)?;
                let stored = self.emit_expr(value)?;
                let Some(stored) = stored else {
                    return Err(Diagnostic::backend_at(
                        "cannot assign void value",
                        value.span(),
                    ));
                };
                let field_ptr = self.emit_struct_field_ptr(struct_ptr, &struct_key, field_index)?;
                build_unit(self.builder.build_store(field_ptr, stored))?;
                let _ = field_ty;
            }
            Stmt::AssignIndex {
                name, index, value, ..
            } => {
                let (array_ptr, elem_ty, len) = {
                    let local = self.locals.get(name).ok_or_else(|| {
                        Diagnostic::backend(format!("unknown variable `{name}`"), 1, 1)
                    })?;
                    let Some((elem_ty, len)) = local.ty.array_elem_len() else {
                        return Err(Diagnostic::backend_at(
                            "index assignment requires array binding",
                            index.span(),
                        ));
                    };
                    (local.pointer, elem_ty.clone(), len)
                };
                let index_value = expect_int_value(self.emit_expr(index)?)?;
                self.emit_bounds_check(index_value, len, index.span())?;
                let stored = self.emit_expr(value)?;
                let Some(stored) = stored else {
                    return Err(Diagnostic::backend_at(
                        "cannot assign void value",
                        value.span(),
                    ));
                };
                let elem_ptr =
                    self.emit_array_element_ptr(array_ptr, &elem_ty, len, index_value)?;
                build_unit(self.builder.build_store(elem_ptr, stored))?;
            }
            Stmt::Return {
                value: Some(expr), ..
            } => {
                let value = self.emit_expr(expr)?;
                let Some(value) = value else {
                    return Err(Diagnostic::backend_at(
                        "cannot return void value",
                        expr.span(),
                    ));
                };
                build_unit(self.builder.build_return(Some(&value)))?;
                self.terminated = true;
            }
            Stmt::Return { value: None, .. } => {
                build_unit(self.builder.build_return(None))?;
                self.terminated = true;
            }
            Stmt::Expr(expr) => {
                self.emit_expr(expr)?;
            }
            Stmt::If {
                condition,
                keyword_span: _,
                then_body,
                else_body,
            } => self.emit_if(condition, then_body, else_body)?,
            Stmt::While {
                condition,
                keyword_span: _,
                body,
            } => self.emit_while(condition, body)?,
            Stmt::Break { keyword_span } => {
                let Some(labels) = self.loop_stack.last() else {
                    return Err(Diagnostic::backend_at(
                        "break outside of loop",
                        *keyword_span,
                    ));
                };
                build_unit(self.builder.build_unconditional_branch(labels.end))?;
                self.terminated = true;
            }
            Stmt::Continue { keyword_span } => {
                let Some(labels) = self.loop_stack.last() else {
                    return Err(Diagnostic::backend_at(
                        "continue outside of loop",
                        *keyword_span,
                    ));
                };
                build_unit(self.builder.build_unconditional_branch(labels.cond))?;
                self.terminated = true;
            }
        }
        Ok(())
    }

    fn emit_while(&mut self, condition: &Expr, body: &[Stmt]) -> XResult<()> {
        let cond_block = self.context.append_basic_block(self.function, "while.cond");
        let body_block = self.context.append_basic_block(self.function, "while.body");
        let end_block = self.context.append_basic_block(self.function, "while.end");

        build_unit(self.builder.build_unconditional_branch(cond_block))?;
        self.loop_stack.push(LoopLabels {
            cond: cond_block,
            end: end_block,
        });

        self.builder.position_at_end(cond_block);
        let condition_value = self.emit_expr(condition)?;
        let condition_value = condition_value
            .ok_or_else(|| {
                Diagnostic::backend_at("while condition cannot be void", condition.span())
            })?
            .into_int_value();
        build_unit(
            self.builder
                .build_conditional_branch(condition_value, body_block, end_block),
        )?;

        self.builder.position_at_end(body_block);
        self.terminated = false;
        for stmt in body {
            self.emit_stmt(stmt)?;
        }
        if !self.terminated {
            build_unit(self.builder.build_unconditional_branch(cond_block))?;
        }

        self.loop_stack.pop();
        self.builder.position_at_end(end_block);
        self.terminated = false;
        Ok(())
    }

    fn emit_array_binding(
        &mut self,
        name: &str,
        elem: &TypeName,
        len: usize,
        value: &Expr,
        source_span: crate::diagnostic::Span,
    ) -> XResult<()> {
        let array_ty = llvm_array_type(self.context, elem, len)?;
        let slot = build_value(self.builder.build_alloca(array_ty, name))?;
        let Expr::ArrayLiteral { elements, .. } = value else {
            return Err(Diagnostic::backend_at(
                "array binding requires array literal initializer",
                source_span,
            ));
        };
        for (index, element) in elements.iter().enumerate() {
            let rendered = self.emit_expr(element)?;
            let Some(rendered) = rendered else {
                return Err(Diagnostic::backend_at(
                    "cannot initialize array with void value",
                    element.span(),
                ));
            };
            let index_value = self.context.i32_type().const_int(index as u64, false);
            let elem_ptr = self.emit_array_element_ptr(slot, elem, len, index_value)?;
            build_unit(self.builder.build_store(elem_ptr, rendered))?;
        }
        self.locals.insert(
            name.to_owned(),
            Local {
                pointer: slot,
                ty: TypeName::Array {
                    elem: Box::new(elem.clone()),
                    len,
                },
                scalar_ty: None,
            },
        );
        Ok(())
    }

    fn emit_struct_binding(
        &mut self,
        name: &str,
        struct_key: &str,
        bound_ty: &TypeName,
        value: &Expr,
        source_span: crate::diagnostic::Span,
    ) -> XResult<()> {
        let struct_ty = self.struct_types.get(struct_key).ok_or_else(|| {
            Diagnostic::backend_at(format!("unknown struct type `{struct_key}`"), source_span)
        })?;
        let slot = build_value(self.builder.build_alloca(*struct_ty, name))?;
        let Expr::StructLiteral { elements, .. } = value else {
            return Err(Diagnostic::backend_at(
                "struct binding requires struct literal initializer",
                source_span,
            ));
        };
        for (index, element) in elements.iter().enumerate() {
            let rendered = self.emit_expr(element)?;
            let Some(rendered) = rendered else {
                return Err(Diagnostic::backend_at(
                    "cannot initialize struct with void value",
                    element.span(),
                ));
            };
            let field_ptr = self.emit_struct_field_ptr(slot, struct_key, index)?;
            build_unit(self.builder.build_store(field_ptr, rendered))?;
        }
        self.locals.insert(
            name.to_owned(),
            Local {
                pointer: slot,
                ty: bound_ty.clone(),
                scalar_ty: None,
            },
        );
        Ok(())
    }

    fn emit_struct_literal_value(
        &mut self,
        struct_key: &str,
        value: &Expr,
        source_span: crate::diagnostic::Span,
    ) -> XResult<Option<BasicValueEnum<'ctx>>> {
        let struct_ty = self.struct_types.get(struct_key).ok_or_else(|| {
            Diagnostic::backend_at(format!("unknown struct type `{struct_key}`"), source_span)
        })?;
        let Expr::StructLiteral { elements, .. } = value else {
            return Err(Diagnostic::backend_at(
                "expected struct literal",
                source_span,
            ));
        };
        let mut aggregate = struct_ty.get_undef();
        for (index, element) in elements.iter().enumerate() {
            let rendered = self.emit_expr(element)?;
            let Some(rendered) = rendered else {
                return Err(Diagnostic::backend_at(
                    "cannot initialize struct with void value",
                    element.span(),
                ));
            };
            aggregate = build_value(self.builder.build_insert_value(
                aggregate,
                rendered,
                index as u32,
                "struct.val",
            ))?
            .into_struct_value();
        }
        Ok(Some(aggregate.as_basic_value_enum()))
    }

    fn emit_struct_from_expr(
        &mut self,
        name: &str,
        struct_key: &str,
        bound_ty: &TypeName,
        value: &Expr,
        source_span: crate::diagnostic::Span,
    ) -> XResult<()> {
        let struct_ty = self.struct_types.get(struct_key).ok_or_else(|| {
            Diagnostic::backend_at(format!("unknown struct type `{struct_key}`"), source_span)
        })?;
        let rendered = self.emit_expr(value)?;
        let Some(rendered) = rendered else {
            return Err(Diagnostic::backend_at(
                "cannot bind void value to struct local",
                source_span,
            ));
        };
        if !rendered.is_struct_value() {
            return Err(Diagnostic::backend_at(
                "struct binding requires struct value initializer",
                source_span,
            ));
        }
        let slot = build_value(self.builder.build_alloca(*struct_ty, name))?;
        build_unit(self.builder.build_store(slot, rendered))?;
        self.locals.insert(
            name.to_owned(),
            Local {
                pointer: slot,
                ty: bound_ty.clone(),
                scalar_ty: None,
            },
        );
        Ok(())
    }

    fn is_enum_constructor_value(&self, value: &Expr, enum_key: &str) -> bool {
        let Expr::Call { callee, .. } = value else {
            return false;
        };
        enum_decl_for_key(self.unit, enum_key)
            .ok()
            .is_some_and(|decl| decl.variants.iter().any(|variant| variant.name == *callee))
    }

    fn emit_enum_from_expr(
        &mut self,
        name: &str,
        enum_key: &str,
        bound_ty: &TypeName,
        value: &Expr,
        source_span: crate::diagnostic::Span,
    ) -> XResult<()> {
        let enum_ty = self.enum_types.get(enum_key).ok_or_else(|| {
            Diagnostic::backend_at(format!("unknown enum type `{enum_key}`"), source_span)
        })?;
        let rendered = self.emit_expr(value)?;
        let Some(rendered) = rendered else {
            return Err(Diagnostic::backend_at(
                "cannot bind void value to enum local",
                source_span,
            ));
        };
        let slot = build_value(self.builder.build_alloca(*enum_ty, name))?;
        build_unit(self.builder.build_store(slot, rendered))?;
        self.locals.insert(
            name.to_owned(),
            Local {
                pointer: slot,
                ty: bound_ty.clone(),
                scalar_ty: None,
            },
        );
        Ok(())
    }

    fn emit_enum_constructor_value(
        &mut self,
        enum_key: &str,
        callee: &str,
        args: &[Expr],
        source_span: crate::diagnostic::Span,
    ) -> XResult<Option<BasicValueEnum<'ctx>>> {
        let enum_ty = *self.enum_types.get(enum_key).ok_or_else(|| {
            Diagnostic::backend_at(format!("unknown enum type `{enum_key}`"), source_span)
        })?;
        let enum_decl = enum_decl_for_key(self.unit, enum_key)?;
        let tag_index = variant_tag_index(enum_decl, callee)?;
        let payload_ty = variant_payload_type(enum_decl, callee)?;
        let i32 = self.context.i32_type();
        let tag_value = i32.const_int(tag_index as u64, false);
        let payload_value = if let Some(payload_ty) = payload_ty {
            if args.len() != 1 {
                return Err(Diagnostic::backend_at(
                    format!("constructor `{callee}` expects 1 argument"),
                    source_span,
                ));
            }
            let rendered = self.emit_expr(&args[0])?;
            let Some(rendered) = rendered else {
                return Err(Diagnostic::backend_at(
                    "cannot initialize enum with void value",
                    args[0].span(),
                ));
            };
            match payload_ty {
                TypeName::I32 => rendered.into_int_value(),
                TypeName::Bool => build_value(self.builder.build_int_z_extend(
                    rendered.into_int_value(),
                    i32,
                    "enum.payload.zext",
                ))?,
                _ => return Err(unsupported_backend_type(source_span)),
            }
        } else {
            if !args.is_empty() {
                return Err(Diagnostic::backend_at(
                    format!("constructor `{callee}` expects 0 arguments"),
                    source_span,
                ));
            }
            i32.const_int(0, false)
        };
        Ok(Some(
            build_enum_value(self.builder, enum_ty, tag_value, payload_value)?
                .as_basic_value_enum(),
        ))
    }

    fn emit_enum_binding(
        &mut self,
        name: &str,
        enum_key: &str,
        bound_ty: &TypeName,
        value: &Expr,
        source_span: crate::diagnostic::Span,
    ) -> XResult<()> {
        let enum_ty = self.enum_types.get(enum_key).ok_or_else(|| {
            Diagnostic::backend_at(format!("unknown enum type `{enum_key}`"), source_span)
        })?;
        let enum_decl = enum_decl_for_key(self.unit, enum_key)?;
        let Expr::Call { callee, args, .. } = value else {
            return Err(Diagnostic::backend_at(
                "enum binding requires variant constructor initializer",
                source_span,
            ));
        };
        let tag_index = variant_tag_index(enum_decl, callee)?;
        let payload_ty = variant_payload_type(enum_decl, callee)?;
        let i32 = self.context.i32_type();
        let tag_value = i32.const_int(tag_index as u64, false);
        let payload_value = if let Some(payload_ty) = payload_ty {
            if args.len() != 1 {
                return Err(Diagnostic::backend_at(
                    format!("constructor `{callee}` expects 1 argument"),
                    source_span,
                ));
            }
            let rendered = self.emit_expr(&args[0])?;
            let Some(rendered) = rendered else {
                return Err(Diagnostic::backend_at(
                    "cannot initialize enum with void value",
                    args[0].span(),
                ));
            };
            match payload_ty {
                TypeName::I32 => rendered.into_int_value(),
                TypeName::Bool => {
                    let bool_val = rendered.into_int_value();
                    build_value(self.builder.build_int_z_extend(
                        bool_val,
                        i32,
                        "enum.payload.zext",
                    ))?
                }
                _ => return Err(unsupported_backend_type(source_span)),
            }
        } else {
            if !args.is_empty() {
                return Err(Diagnostic::backend_at(
                    format!("constructor `{callee}` expects 0 arguments"),
                    source_span,
                ));
            }
            i32.const_int(0, false)
        };
        let aggregate = build_enum_value(self.builder, *enum_ty, tag_value, payload_value)?;
        let slot = build_value(self.builder.build_alloca(*enum_ty, name))?;
        build_unit(self.builder.build_store(slot, aggregate))?;
        self.locals.insert(
            name.to_owned(),
            Local {
                pointer: slot,
                ty: bound_ty.clone(),
                scalar_ty: None,
            },
        );
        Ok(())
    }

    fn emit_struct_field_ptr(
        &mut self,
        struct_ptr: PointerValue<'ctx>,
        struct_key: &str,
        field_index: usize,
    ) -> XResult<PointerValue<'ctx>> {
        let struct_ty = self.struct_types.get(struct_key).ok_or_else(|| {
            Diagnostic::backend(format!("unknown struct type `{struct_key}`"), 1, 1)
        })?;
        let zero = self.context.i32_type().const_int(0, false);
        let index = self.context.i32_type().const_int(field_index as u64, false);
        build_value(unsafe {
            self.builder
                .build_gep(*struct_ty, struct_ptr, &[zero, index], "struct.field")
        })
    }

    fn emit_bounds_check(
        &mut self,
        index: inkwell::values::IntValue<'ctx>,
        len: usize,
        _span: crate::diagnostic::Span,
    ) -> XResult<()> {
        let i32 = self.context.i32_type();
        let zero = i32.const_int(0, true);
        let ge = build_value(self.builder.build_int_compare(
            IntPredicate::SGE,
            index,
            zero,
            "bounds.ge",
        ))?;
        let len_const = i32.const_int(len as u64, false);
        let lt = build_value(self.builder.build_int_compare(
            IntPredicate::SLT,
            index,
            len_const,
            "bounds.lt",
        ))?;
        let ok = build_value(self.builder.build_and(ge, lt, "bounds.ok"))?;

        let ok_block = self.context.append_basic_block(self.function, "bounds.ok");
        let trap_block = self
            .context
            .append_basic_block(self.function, "bounds.trap");
        let resume_block = self
            .context
            .append_basic_block(self.function, "bounds.resume");

        build_unit(
            self.builder
                .build_conditional_branch(ok, ok_block, trap_block),
        )?;

        self.builder.position_at_end(trap_block);
        let trap = self.module.get_function("llvm.trap").unwrap_or_else(|| {
            let fn_type = self.context.void_type().fn_type(&[], false);
            self.module.add_function("llvm.trap", fn_type, None)
        });
        build_unit(self.builder.build_call(trap, &[], "trap"))?;
        build_unit(self.builder.build_unreachable())?;

        self.builder.position_at_end(ok_block);
        build_unit(self.builder.build_unconditional_branch(resume_block))?;
        self.builder.position_at_end(resume_block);
        Ok(())
    }

    fn emit_array_element_ptr(
        &mut self,
        array_ptr: PointerValue<'ctx>,
        elem: &TypeName,
        len: usize,
        index: inkwell::values::IntValue<'ctx>,
    ) -> XResult<PointerValue<'ctx>> {
        let array_ty = llvm_array_type(self.context, elem, len)?;
        let zero = self.context.i32_type().const_int(0, false);
        build_value(unsafe {
            self.builder
                .build_gep(array_ty, array_ptr, &[zero, index], "array.elem")
        })
    }

    fn emit_if(&mut self, condition: &Expr, then_body: &[Stmt], else_body: &[Stmt]) -> XResult<()> {
        let condition_span = condition.span();
        let condition = self.emit_expr(condition)?;
        let condition = condition
            .ok_or_else(|| Diagnostic::backend_at("if condition cannot be void", condition_span))?
            .into_int_value();

        let then_block = self.context.append_basic_block(self.function, "if.then");
        let else_block = self.context.append_basic_block(self.function, "if.else");
        let end_block = self.context.append_basic_block(self.function, "if.end");
        build_unit(
            self.builder
                .build_conditional_branch(condition, then_block, else_block),
        )?;

        self.builder.position_at_end(then_block);
        self.terminated = false;
        for stmt in then_body {
            self.emit_stmt(stmt)?;
        }
        let then_terminated = self.terminated;
        if !then_terminated {
            build_unit(self.builder.build_unconditional_branch(end_block))?;
        }

        self.builder.position_at_end(else_block);
        self.terminated = false;
        for stmt in else_body {
            self.emit_stmt(stmt)?;
        }
        let else_terminated = self.terminated;
        if !else_terminated {
            build_unit(self.builder.build_unconditional_branch(end_block))?;
        }

        if then_terminated && else_terminated {
            self.builder.position_at_end(end_block);
            build_unit(self.builder.build_unreachable())?;
            self.terminated = true;
        } else {
            self.builder.position_at_end(end_block);
            self.terminated = false;
        }
        Ok(())
    }

    fn resolve_array_base(&self, expr: &Expr) -> XResult<(PointerValue<'ctx>, TypeName, usize)> {
        match expr {
            Expr::Variable { name, span } => {
                let local = self.locals.get(name).ok_or_else(|| {
                    Diagnostic::backend_at(format!("unknown variable `{name}`"), *span)
                })?;
                let Some((elem, len)) = local.ty.array_elem_len() else {
                    return Err(Diagnostic::backend_at(
                        "index base must be an array binding",
                        *span,
                    ));
                };
                Ok((local.pointer, elem.clone(), len))
            }
            _ => Err(Diagnostic::backend_at(
                "index base must be a variable",
                expr.span(),
            )),
        }
    }

    fn emit_expr(&mut self, expr: &Expr) -> XResult<Option<BasicValueEnum<'ctx>>> {
        match expr {
            Expr::Integer { value, .. } => Ok(Some(
                self.context
                    .i32_type()
                    .const_int(*value as u64, true)
                    .as_basic_value_enum(),
            )),
            Expr::Bool { value, .. } => Ok(Some(
                self.context
                    .bool_type()
                    .const_int(u64::from(*value), false)
                    .as_basic_value_enum(),
            )),
            Expr::String { span, .. } => Err(unsupported_backend_type(*span)),
            Expr::Variable { name, span } => {
                let local = self.locals.get(name).ok_or_else(|| {
                    Diagnostic::backend(format!("unknown variable `{name}`"), 1, 1)
                })?;
                if local.ty.enum_ref().is_some_and(|enum_ref| {
                    let key = enum_key_from_ref(self.module_name, enum_ref);
                    self.enum_types.contains_key(&key)
                }) {
                    let enum_key =
                        enum_key_from_type(self.module_name, &local.ty).expect("enum key");
                    let enum_ty = *self.enum_types.get(&enum_key).expect("enum type");
                    return Ok(Some(build_value(self.builder.build_load(
                        enum_ty,
                        local.pointer,
                        &format!("{name}.load"),
                    ))?));
                }
                if let Some(struct_key) = struct_key_from_type(self.module_name, &local.ty)
                    && let Some(struct_ty) = self.struct_types.get(&struct_key)
                {
                    return Ok(Some(build_value(self.builder.build_load(
                        *struct_ty,
                        local.pointer,
                        &format!("{name}.load"),
                    ))?));
                }
                let Some(scalar_ty) = local.scalar_ty else {
                    if local.ty.array_elem_len().is_some() {
                        return Err(Diagnostic::backend_at(
                            "cannot use array binding as scalar value",
                            *span,
                        ));
                    }
                    if local.ty.enum_ref().is_some_and(|enum_ref| {
                        let key = enum_key_from_ref(self.module_name, enum_ref);
                        self.enum_types.contains_key(&key)
                    }) {
                        return Err(Diagnostic::backend_at(
                            "cannot use enum binding as scalar value",
                            *span,
                        ));
                    }
                    return Err(Diagnostic::backend_at(
                        "cannot use struct binding as scalar value",
                        *span,
                    ));
                };
                Ok(Some(build_value(self.builder.build_load(
                    scalar_ty,
                    local.pointer,
                    &format!("{name}.load"),
                ))?))
            }
            Expr::Call { callee, args, span } => {
                if let Some(enum_ref) = self.function_return_type.enum_ref() {
                    let enum_key = enum_key_from_ref(self.module_name, enum_ref);
                    if self.is_enum_constructor_value(
                        &Expr::Call {
                            callee: callee.clone(),
                            args: args.to_vec(),
                            span: *span,
                        },
                        &enum_key,
                    ) {
                        return self.emit_enum_constructor_value(&enum_key, callee, args, *span);
                    }
                }
                self.emit_call(
                    &mangle_function(
                        self.module_name,
                        callee,
                        self.module_name == self.unit.entry && callee == "main",
                    ),
                    args,
                    false,
                )
            }
            Expr::QualifiedCall {
                module,
                callee,
                args,
                ..
            } => self.emit_call(
                &mangle_function(
                    module,
                    callee,
                    module == &self.unit.entry && callee == "main",
                ),
                args,
                true,
            ),
            Expr::Unary { op, expr, .. } => {
                if *op == UnaryOp::Negate
                    && let Expr::Integer { value, .. } = expr.as_ref()
                    && *value == i64::from(i32::MAX) + 1
                {
                    return Ok(Some(
                        self.context
                            .i32_type()
                            .const_int(i32::MIN as i64 as u64, true)
                            .as_basic_value_enum(),
                    ));
                }
                let value = self.emit_expr(expr)?;
                let int_value = expect_int_value(value)?;
                match op {
                    UnaryOp::Negate => Ok(Some(
                        build_value(self.builder.build_int_neg(int_value, "negtmp"))?
                            .as_basic_value_enum(),
                    )),
                    UnaryOp::Not => Ok(Some(
                        build_value(self.builder.build_not(int_value, "nottmp"))?
                            .as_basic_value_enum(),
                    )),
                }
            }
            Expr::Binary {
                left, op, right, ..
            } => match op {
                BinaryOp::And => self.emit_short_circuit_and(left, right),
                BinaryOp::Or => self.emit_short_circuit_or(left, right),
                _ => self.emit_binary(left, *op, right),
            },
            Expr::ArrayLiteral { span, .. } => Err(Diagnostic::backend_at(
                "array literals are only supported in array bindings",
                *span,
            )),
            Expr::Index {
                base,
                index,
                span: _,
            } => {
                let (array_ptr, elem_ty, len) = self.resolve_array_base(base)?;
                let index_value = expect_int_value(self.emit_expr(index)?)?;
                self.emit_bounds_check(index_value, len, index.span())?;
                let elem_ptr =
                    self.emit_array_element_ptr(array_ptr, &elem_ty, len, index_value)?;
                let scalar_ty = llvm_scalar_type(self.context, &elem_ty, index.span())?;
                Ok(Some(build_value(self.builder.build_load(
                    scalar_ty,
                    elem_ptr,
                    "index.load",
                ))?))
            }
            Expr::StructLiteral { span, .. } => {
                if let Some(struct_key) =
                    struct_key_from_type(self.module_name, &self.function_return_type)
                    && self.struct_types.contains_key(&struct_key)
                {
                    return self.emit_struct_literal_value(&struct_key, expr, *span);
                }
                Err(Diagnostic::backend_at(
                    "struct literals require an expected struct type",
                    *span,
                ))
            }
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
                    return Err(Diagnostic::backend_at(
                        "field access base must be a variable",
                        base.span(),
                    ));
                };
                let (struct_ptr, struct_key) = {
                    let local = self.locals.get(name).ok_or_else(|| {
                        Diagnostic::backend_at(format!("unknown variable `{name}`"), *base_span)
                    })?;
                    if local.ty.enum_ref().is_some_and(|enum_ref| {
                        let key = enum_key_from_ref(self.module_name, enum_ref);
                        self.enum_types.contains_key(&key)
                    }) {
                        return Err(Diagnostic::backend_at(
                            "field access requires struct binding",
                            *field_span,
                        ));
                    }
                    if local.ty.struct_ref().is_none() {
                        return Err(Diagnostic::backend_at(
                            "field access requires struct binding",
                            *field_span,
                        ));
                    }
                    let struct_key =
                        struct_key_from_type(self.module_name, &local.ty).expect("struct key");
                    (local.pointer, struct_key)
                };
                let field_index = struct_field_index_for_unit(self.unit, &struct_key, field)?;
                let field_ty = struct_field_type_for_unit(self.unit, &struct_key, field_index)?;
                let field_ptr = self.emit_struct_field_ptr(struct_ptr, &struct_key, field_index)?;
                let scalar_ty = llvm_scalar_type(self.context, &field_ty, *field_span)?;
                Ok(Some(build_value(self.builder.build_load(
                    scalar_ty,
                    field_ptr,
                    "field.load",
                ))?))
            }
            Expr::Match {
                scrutinee,
                arms,
                span,
            } => self.emit_match(scrutinee, arms, *span),
        }
    }

    fn emit_match(
        &mut self,
        scrutinee: &Expr,
        arms: &[MatchArm],
        span: crate::diagnostic::Span,
    ) -> XResult<Option<BasicValueEnum<'ctx>>> {
        let scrutinee_ty = self.type_name_for_scrutinee(scrutinee)?;
        let enum_key = enum_key_from_type(self.module_name, &scrutinee_ty).ok_or_else(|| {
            Diagnostic::backend_at("match scrutinee must be an enum type", scrutinee.span())
        })?;
        let enum_ty = *self.enum_types.get(&enum_key).ok_or_else(|| {
            Diagnostic::backend_at(format!("unknown enum type `{enum_key}`"), span)
        })?;
        let enum_decl = enum_decl_for_key(self.unit, &enum_key)?;
        let loaded = self.emit_enum_struct_value(scrutinee, &enum_key, enum_ty)?;
        let tag =
            build_value(self.builder.build_extract_value(loaded, 0, "match.tag"))?.into_int_value();
        let payload_raw =
            build_value(self.builder.build_extract_value(loaded, 1, "match.payload"))?
                .into_int_value();

        let pre_match_block = current_block(self.builder)?;
        let merge_block = self.context.append_basic_block(self.function, "match.end");
        let trap_block = self.context.append_basic_block(self.function, "match.trap");

        let mut wildcard_block: Option<BasicBlock<'ctx>> = None;
        let mut arm_entries: Vec<(MatchArm, BasicBlock<'ctx>, Option<usize>)> = Vec::new();
        for (index, arm) in arms.iter().enumerate() {
            let block = self
                .context
                .append_basic_block(self.function, &format!("match.arm.{index}"));
            match &arm.pattern {
                Pattern::Wildcard { .. } => {
                    if wildcard_block.is_some() {
                        return Err(Diagnostic::backend_at(
                            "duplicate match arm",
                            arm.pattern.span(),
                        ));
                    }
                    wildcard_block = Some(block);
                    arm_entries.push((arm.clone(), block, None));
                }
                Pattern::Variant { name, .. } => {
                    let tag_index = variant_tag_index(enum_decl, name)?;
                    arm_entries.push((arm.clone(), block, Some(tag_index)));
                }
            }
        }

        let default_block = wildcard_block.unwrap_or(trap_block);
        let i32 = self.context.i32_type();
        let mut switch_cases = Vec::new();
        for (arm, block, tag_index) in &arm_entries {
            if let Some(tag_index) = tag_index {
                switch_cases.push((i32.const_int(*tag_index as u64, false), *block));
                let _ = arm;
            }
        }
        build_value(self.builder.build_switch(tag, default_block, &switch_cases))?;

        self.builder.position_at_end(trap_block);
        let trap = self.module.get_function("llvm.trap").unwrap_or_else(|| {
            let fn_type = self.context.void_type().fn_type(&[], false);
            self.module.add_function("llvm.trap", fn_type, None)
        });
        build_unit(self.builder.build_call(trap, &[], "trap"))?;
        build_unit(self.builder.build_unreachable())?;

        let mut phi_entries: Vec<(BasicValueEnum<'ctx>, BasicBlock<'ctx>)> = Vec::new();
        let mut result_llvm_ty: Option<BasicTypeEnum<'ctx>> = None;

        for (arm, block, _) in arm_entries {
            self.builder.position_at_end(block);
            self.terminated = false;
            let saved_locals = self.locals.clone();

            if let Pattern::Variant {
                name,
                binding: Some(binding),
                ..
            } = &arm.pattern
            {
                if binding != "_"
                    && let Some(payload_ty) = variant_payload_type(enum_decl, name)?
                {
                    let payload_value = match payload_ty {
                        TypeName::I32 => payload_raw.as_basic_value_enum(),
                        TypeName::Bool => build_value(self.builder.build_int_truncate(
                            payload_raw,
                            self.context.bool_type(),
                            "match.payload.trunc",
                        ))?
                        .as_basic_value_enum(),
                        _ => return Err(unsupported_backend_type(arm.pattern.span())),
                    };
                    let llvm_ty = llvm_scalar_type(self.context, &payload_ty, arm.pattern.span())?;
                    let slot = build_value(self.builder.build_alloca(llvm_ty, binding.as_str()))?;
                    build_unit(self.builder.build_store(slot, payload_value))?;
                    self.locals.insert(
                        binding.clone(),
                        Local {
                            pointer: slot,
                            ty: payload_ty.clone(),
                            scalar_ty: Some(llvm_ty),
                        },
                    );
                }
            }

            let result = self.emit_match_body(&arm.body)?;
            let Some(result) = result else {
                return Err(Diagnostic::backend_at(
                    "match arm cannot produce void",
                    arm.body.span(),
                ));
            };
            let llvm_ty = result.get_type();
            if let Some(expected) = result_llvm_ty {
                if expected != llvm_ty {
                    return Err(Diagnostic::backend_at(
                        "match arm type mismatch in codegen",
                        arm.body.span(),
                    ));
                }
            } else {
                result_llvm_ty = Some(llvm_ty);
            }
            let arm_end = current_block(self.builder)?;
            if !self.terminated {
                build_unit(self.builder.build_unconditional_branch(merge_block))?;
            }
            phi_entries.push((result, arm_end));
            self.locals = saved_locals;
        }

        self.builder.position_at_end(merge_block);
        self.terminated = false;
        let Some(result_llvm_ty) = result_llvm_ty else {
            return Err(Diagnostic::backend_at(
                "match must have at least one arm",
                span,
            ));
        };
        let result = if phi_entries.len() == 1 {
            phi_entries[0].0
        } else {
            let phi = build_value(self.builder.build_phi(result_llvm_ty, "match.result"))?;
            let incoming: Vec<(&dyn BasicValue<'ctx>, BasicBlock<'ctx>)> = phi_entries
                .iter()
                .map(|(value, block)| (value as &dyn BasicValue<'ctx>, *block))
                .collect();
            phi.add_incoming(&incoming);
            phi.as_basic_value()
        };
        let temp = build_value(self.builder.build_alloca(result_llvm_ty, "match.tmp"))?;
        build_unit(self.builder.build_store(temp, result))?;
        let continue_block = self.context.append_basic_block(self.function, "match.cont");
        build_unit(self.builder.build_unconditional_branch(continue_block))?;
        self.builder.position_at_end(continue_block);
        let loaded = build_value(self.builder.build_load(result_llvm_ty, temp, "match.val"))?;
        let _ = pre_match_block;
        Ok(Some(loaded))
    }

    fn type_name_for_scrutinee(&self, expr: &Expr) -> XResult<TypeName> {
        match expr {
            Expr::Variable { name, span } => {
                let local = self.locals.get(name).ok_or_else(|| {
                    Diagnostic::backend_at(format!("unknown variable `{name}`"), *span)
                })?;
                Ok(local.ty.clone())
            }
            Expr::Call { callee, .. } => self.function_return_type_name(callee),
            Expr::QualifiedCall { module, callee, .. } => {
                self.external_function_return_type_name(module, callee)
            }
            _ => Err(Diagnostic::backend_at(
                "match scrutinee must be an enum value",
                expr.span(),
            )),
        }
    }

    fn function_return_type_name(&self, callee: &str) -> XResult<TypeName> {
        self.unit
            .modules
            .get(self.module_name)
            .and_then(|module| {
                module
                    .program
                    .functions
                    .iter()
                    .find(|function| function.name == callee)
            })
            .map(|function| function.return_type.clone())
            .ok_or_else(|| Diagnostic::backend(format!("unknown function `{callee}`"), 1, 1))
    }

    fn external_function_return_type_name(&self, module: &str, callee: &str) -> XResult<TypeName> {
        self.unit
            .modules
            .get(module)
            .and_then(|loaded| {
                loaded
                    .program
                    .functions
                    .iter()
                    .find(|function| function.name == callee)
            })
            .map(|function| {
                if module == self.module_name {
                    function.return_type.clone()
                } else if let TypeName::Named(name) = &function.return_type {
                    if enum_decl_for_key(self.unit, &enum_ir_name(module, name)).is_ok() {
                        TypeName::Qualified {
                            module: module.to_owned(),
                            name: name.clone(),
                        }
                    } else {
                        function.return_type.clone()
                    }
                } else {
                    function.return_type.clone()
                }
            })
            .ok_or_else(|| {
                Diagnostic::backend(format!("unknown function `{module}.{callee}`"), 1, 1)
            })
    }

    fn emit_enum_struct_value(
        &mut self,
        expr: &Expr,
        enum_key: &str,
        enum_ty: StructType<'ctx>,
    ) -> XResult<inkwell::values::StructValue<'ctx>> {
        if let Expr::Variable { name, span } = expr {
            let local = self.locals.get(name).ok_or_else(|| {
                Diagnostic::backend_at(format!("unknown variable `{name}`"), *span)
            })?;
            return Ok(build_value(
                self.builder
                    .build_load(enum_ty, local.pointer, "match.load"),
            )?
            .into_struct_value());
        }
        let value = self.emit_expr(expr)?;
        let Some(value) = value else {
            return Err(Diagnostic::backend_at(
                "match scrutinee cannot be void",
                expr.span(),
            ));
        };
        if !value.is_struct_value() {
            return Err(Diagnostic::backend_at(
                format!("match scrutinee must be enum type `{enum_key}`"),
                expr.span(),
            ));
        }
        Ok(value.into_struct_value())
    }

    fn emit_match_body(&mut self, body: &MatchBody) -> XResult<Option<BasicValueEnum<'ctx>>> {
        match body {
            MatchBody::Expr(expr) => self.emit_expr(expr),
            MatchBody::Block(stmts) => {
                if stmts.is_empty() {
                    return Err(Diagnostic::backend_at(
                        "match arm block must end with an expression statement",
                        body.span(),
                    ));
                }
                let (prefix, last) = stmts.split_at(stmts.len() - 1);
                for stmt in prefix {
                    self.emit_stmt(stmt)?;
                    if self.terminated {
                        return Ok(None);
                    }
                }
                let Stmt::Expr(expr) = &last[0] else {
                    return Err(Diagnostic::backend_at(
                        "match arm block must end with an expression statement",
                        body.span(),
                    ));
                };
                self.emit_expr(expr)
            }
        }
    }

    fn emit_call(
        &mut self,
        symbol: &str,
        args: &[Expr],
        _external: bool,
    ) -> XResult<Option<BasicValueEnum<'ctx>>> {
        let function = self
            .functions
            .get(symbol)
            .copied()
            .ok_or_else(|| Diagnostic::backend(format!("unknown function `{symbol}`"), 1, 1))?;
        let mut rendered = Vec::new();
        for arg in args {
            let value = self.emit_expr(arg)?;
            let Some(value) = value else {
                return Err(Diagnostic::backend("cannot pass void as an argument", 1, 1));
            };
            rendered.push(BasicMetadataValueEnum::from(value));
        }
        let call = build_value(self.builder.build_call(function, &rendered, "calltmp"))?;
        if function.get_type().get_return_type().is_none() {
            Ok(None)
        } else {
            Ok(Some(call.try_as_basic_value().unwrap_basic()))
        }
    }

    fn emit_binary(
        &mut self,
        left: &Expr,
        op: BinaryOp,
        right: &Expr,
    ) -> XResult<Option<BasicValueEnum<'ctx>>> {
        let left = expect_int_value(self.emit_expr(left)?)?;
        let right = expect_int_value(self.emit_expr(right)?)?;
        let value = match op {
            BinaryOp::Add => build_value(self.builder.build_int_add(left, right, "addtmp"))?
                .as_basic_value_enum(),
            BinaryOp::Subtract => build_value(self.builder.build_int_sub(left, right, "subtmp"))?
                .as_basic_value_enum(),
            BinaryOp::Multiply => build_value(self.builder.build_int_mul(left, right, "multmp"))?
                .as_basic_value_enum(),
            BinaryOp::Divide => {
                build_value(self.builder.build_int_signed_div(left, right, "divtmp"))?
                    .as_basic_value_enum()
            }
            BinaryOp::Remainder => {
                build_value(self.builder.build_int_signed_rem(left, right, "remtmp"))?
                    .as_basic_value_enum()
            }
            BinaryOp::Equal => {
                build_value(
                    self.builder
                        .build_int_compare(IntPredicate::EQ, left, right, "eqtmp"),
                )?
                .as_basic_value_enum()
            }
            BinaryOp::NotEqual => {
                build_value(
                    self.builder
                        .build_int_compare(IntPredicate::NE, left, right, "netmp"),
                )?
                .as_basic_value_enum()
            }
            BinaryOp::Less => build_value(self.builder.build_int_compare(
                IntPredicate::SLT,
                left,
                right,
                "lttmp",
            ))?
            .as_basic_value_enum(),
            BinaryOp::LessEqual => build_value(self.builder.build_int_compare(
                IntPredicate::SLE,
                left,
                right,
                "letmp",
            ))?
            .as_basic_value_enum(),
            BinaryOp::Greater => build_value(self.builder.build_int_compare(
                IntPredicate::SGT,
                left,
                right,
                "gttmp",
            ))?
            .as_basic_value_enum(),
            BinaryOp::GreaterEqual => build_value(self.builder.build_int_compare(
                IntPredicate::SGE,
                left,
                right,
                "getmp",
            ))?
            .as_basic_value_enum(),
            BinaryOp::And | BinaryOp::Or => unreachable!("logical operators lower separately"),
        };
        Ok(Some(value))
    }

    fn emit_short_circuit_and(
        &mut self,
        left: &Expr,
        right: &Expr,
    ) -> XResult<Option<BasicValueEnum<'ctx>>> {
        let left_value = expect_int_value(self.emit_expr(left)?)?;
        let left_block = self
            .builder
            .get_insert_block()
            .ok_or_else(|| Diagnostic::backend("LLVM builder is not positioned", 1, 1))?;
        let rhs_block = self.context.append_basic_block(self.function, "and.rhs");
        let end_block = self.context.append_basic_block(self.function, "and.end");
        build_unit(
            self.builder
                .build_conditional_branch(left_value, rhs_block, end_block),
        )?;

        self.builder.position_at_end(rhs_block);
        let right_value = expect_int_value(self.emit_expr(right)?)?;
        let rhs_end_block = current_block(self.builder)?;
        build_unit(self.builder.build_unconditional_branch(end_block))?;

        self.builder.position_at_end(end_block);
        let phi = build_value(self.builder.build_phi(self.context.bool_type(), "andtmp"))?;
        let false_value = self.context.bool_type().const_int(0, false);
        phi.add_incoming(&[(&false_value, left_block), (&right_value, rhs_end_block)]);
        Ok(Some(phi.as_basic_value()))
    }

    fn emit_short_circuit_or(
        &mut self,
        left: &Expr,
        right: &Expr,
    ) -> XResult<Option<BasicValueEnum<'ctx>>> {
        let left_value = expect_int_value(self.emit_expr(left)?)?;
        let left_block = self
            .builder
            .get_insert_block()
            .ok_or_else(|| Diagnostic::backend("LLVM builder is not positioned", 1, 1))?;
        let rhs_block = self.context.append_basic_block(self.function, "or.rhs");
        let end_block = self.context.append_basic_block(self.function, "or.end");
        build_unit(
            self.builder
                .build_conditional_branch(left_value, end_block, rhs_block),
        )?;

        self.builder.position_at_end(rhs_block);
        let right_value = expect_int_value(self.emit_expr(right)?)?;
        let rhs_end_block = current_block(self.builder)?;
        build_unit(self.builder.build_unconditional_branch(end_block))?;

        self.builder.position_at_end(end_block);
        let phi = build_value(self.builder.build_phi(self.context.bool_type(), "ortmp"))?;
        let true_value = self.context.bool_type().const_int(1, false);
        phi.add_incoming(&[(&true_value, left_block), (&right_value, rhs_end_block)]);
        Ok(Some(phi.as_basic_value()))
    }
}

fn ensure_stmt_supported(unit: &CompilationUnit, module_name: &str, stmt: &Stmt) -> XResult<()> {
    match stmt {
        Stmt::Let {
            value, annotation, ..
        } => {
            if let Some(ty) = annotation {
                ensure_local_type_supported(unit, module_name, ty, value.span())?;
            }
            ensure_expr_supported(value)
        }
        Stmt::Assign { value, .. }
        | Stmt::Return {
            value: Some(value), ..
        }
        | Stmt::Expr(value) => ensure_expr_supported(value),
        Stmt::Return { value: None, .. } => Ok(()),
        Stmt::Break { .. } | Stmt::Continue { .. } => Ok(()),
        Stmt::While {
            condition, body, ..
        } => {
            ensure_expr_supported(condition)?;
            for stmt in body {
                ensure_stmt_supported(unit, module_name, stmt)?;
            }
            Ok(())
        }
        Stmt::AssignField { value, .. } => ensure_expr_supported(value),
        Stmt::AssignIndex { index, value, .. } => {
            ensure_expr_supported(index)?;
            ensure_expr_supported(value)
        }
        Stmt::If {
            condition,
            keyword_span: _,
            then_body,
            else_body,
        } => {
            ensure_expr_supported(condition)?;
            for stmt in then_body.iter().chain(else_body.iter()) {
                ensure_stmt_supported(unit, module_name, stmt)?;
            }
            Ok(())
        }
    }
}

fn ensure_expr_supported(expr: &Expr) -> XResult<()> {
    match expr {
        Expr::String { span, .. } => Err(unsupported_backend_type(*span)),
        Expr::Call { args, .. } | Expr::QualifiedCall { args, .. } => {
            for arg in args {
                ensure_expr_supported(arg)?;
            }
            Ok(())
        }
        Expr::Unary { expr, .. } => ensure_expr_supported(expr),
        Expr::Binary { left, right, .. } => {
            ensure_expr_supported(left)?;
            ensure_expr_supported(right)
        }
        Expr::Integer { .. } | Expr::Bool { .. } | Expr::Variable { .. } => Ok(()),
        Expr::ArrayLiteral { elements, .. } => {
            for element in elements {
                ensure_expr_supported(element)?;
            }
            Ok(())
        }
        Expr::StructLiteral { elements, .. } => {
            for element in elements {
                ensure_expr_supported(element)?;
            }
            Ok(())
        }
        Expr::FieldAccess { base, .. } => ensure_expr_supported(base),
        Expr::Index { base, index, .. } => {
            ensure_expr_supported(base)?;
            ensure_expr_supported(index)
        }
        Expr::Match {
            scrutinee, arms, ..
        } => {
            ensure_expr_supported(scrutinee)?;
            for arm in arms {
                ensure_match_body_supported(arm)?;
            }
            Ok(())
        }
    }
}

fn ensure_match_body_supported(arm: &MatchArm) -> XResult<()> {
    match &arm.body {
        MatchBody::Expr(expr) => ensure_expr_supported(expr),
        MatchBody::Block(stmts) => {
            for stmt in stmts {
                ensure_stmt_supported_in_match(stmt)?;
            }
            Ok(())
        }
    }
}

fn ensure_stmt_supported_in_match(stmt: &Stmt) -> XResult<()> {
    match stmt {
        Stmt::Let { value, .. } => ensure_expr_supported(value),
        Stmt::Assign { value, .. }
        | Stmt::Return {
            value: Some(value), ..
        }
        | Stmt::Expr(value) => ensure_expr_supported(value),
        Stmt::Return { value: None, .. } => Ok(()),
        Stmt::Break { .. } | Stmt::Continue { .. } => Ok(()),
        Stmt::While {
            condition, body, ..
        } => {
            ensure_expr_supported(condition)?;
            for stmt in body {
                ensure_stmt_supported_in_match(stmt)?;
            }
            Ok(())
        }
        Stmt::AssignField { value, .. } => ensure_expr_supported(value),
        Stmt::AssignIndex { index, value, .. } => {
            ensure_expr_supported(index)?;
            ensure_expr_supported(value)
        }
        Stmt::If {
            condition,
            then_body,
            else_body,
            ..
        } => {
            ensure_expr_supported(condition)?;
            for stmt in then_body.iter().chain(else_body.iter()) {
                ensure_stmt_supported_in_match(stmt)?;
            }
            Ok(())
        }
    }
}

fn ensure_enum_payload_backend_type(ty: &TypeName, span: crate::diagnostic::Span) -> XResult<()> {
    match ty {
        TypeName::I32 | TypeName::Bool => Ok(()),
        _ => Err(unsupported_backend_type(span)),
    }
}

fn ensure_struct_field_backend_type(ty: &TypeName, span: crate::diagnostic::Span) -> XResult<()> {
    match ty {
        TypeName::I32 | TypeName::Bool => Ok(()),
        TypeName::Str => Err(Diagnostic::backend_at(
            "LLVM backend does not support struct field type str",
            span,
        )),
        TypeName::Named(_) | TypeName::Qualified { .. } => Err(Diagnostic::backend_at(
            "LLVM backend does not support nested struct fields yet",
            span,
        )),
        TypeName::Void | TypeName::Array { .. } => Err(unsupported_backend_type(span)),
    }
}

fn ensure_array_element_backend_type(
    elem: &TypeName,
    span: crate::diagnostic::Span,
) -> XResult<()> {
    match elem {
        TypeName::I32 | TypeName::Bool => Ok(()),
        TypeName::Str
        | TypeName::Named(_)
        | TypeName::Qualified { .. }
        | TypeName::Void
        | TypeName::Array { .. } => Err(unsupported_backend_type(span)),
    }
}

fn ensure_local_type_supported(
    unit: &CompilationUnit,
    module_name: &str,
    ty: &TypeName,
    span: crate::diagnostic::Span,
) -> XResult<()> {
    match ty {
        TypeName::I32 | TypeName::Bool | TypeName::Void => Ok(()),
        TypeName::Array { elem, .. } => ensure_array_element_backend_type(elem, span),
        TypeName::Named(name) => {
            let key = struct_ir_name(module_name, name);
            if struct_decl_for_key(unit, &key).is_ok() {
                let struct_decl = struct_decl_for_key(unit, &key)?;
                for field in &struct_decl.fields {
                    ensure_struct_field_backend_type(&field.ty, field.ty_span)?;
                }
                return Ok(());
            }
            let enum_key = enum_ir_name(module_name, name);
            let enum_decl = enum_decl_for_key(unit, &enum_key)?;
            for variant in &enum_decl.variants {
                if let Some(payload) = &variant.payload {
                    ensure_enum_payload_backend_type(&payload.ty, payload.ty_span)?;
                }
            }
            Ok(())
        }
        TypeName::Qualified { module, name } => {
            let key = struct_ir_name(module, name);
            if struct_decl_for_key(unit, &key).is_ok() {
                let struct_decl = struct_decl_for_key(unit, &key)?;
                for field in &struct_decl.fields {
                    ensure_struct_field_backend_type(&field.ty, field.ty_span)?;
                }
                return Ok(());
            }
            let enum_key = enum_ir_name(module, name);
            let enum_decl = enum_decl_for_key(unit, &enum_key)?;
            for variant in &enum_decl.variants {
                if let Some(payload) = &variant.payload {
                    ensure_enum_payload_backend_type(&payload.ty, payload.ty_span)?;
                }
            }
            Ok(())
        }
        TypeName::Str => Err(unsupported_backend_type(span)),
    }
}

fn struct_key_from_ref(module_name: &str, struct_ref: StructRef<'_>) -> String {
    match struct_ref {
        StructRef::Local(name) => struct_ir_name(module_name, name),
        StructRef::Qualified { module, name } => struct_ir_name(module, name),
    }
}

fn struct_field_index_for_unit(
    unit: &CompilationUnit,
    struct_key: &str,
    field: &str,
) -> XResult<usize> {
    let struct_decl = struct_decl_for_key(unit, struct_key)?;
    struct_decl
        .fields
        .iter()
        .position(|f| f.name == field)
        .ok_or_else(|| {
            Diagnostic::backend(
                format!("struct `{struct_key}` has no field `{field}`"),
                1,
                1,
            )
        })
}

fn struct_field_type_for_unit(
    unit: &CompilationUnit,
    struct_key: &str,
    field_index: usize,
) -> XResult<TypeName> {
    let struct_decl = struct_decl_for_key(unit, struct_key)?;
    struct_decl
        .fields
        .get(field_index)
        .map(|field| field.ty.clone())
        .ok_or_else(|| Diagnostic::backend(format!("invalid field index for `{struct_key}`"), 1, 1))
}

fn struct_decl_for_key<'a>(
    unit: &'a CompilationUnit,
    struct_key: &str,
) -> XResult<&'a crate::ast::StructDecl> {
    let (module_name, struct_name) = struct_key
        .split_once('.')
        .ok_or_else(|| Diagnostic::backend(format!("invalid struct key `{struct_key}`"), 1, 1))?;
    let module = unit
        .modules
        .get(module_name)
        .ok_or_else(|| Diagnostic::backend(format!("unknown module `{module_name}`"), 1, 1))?;
    module
        .program
        .structs
        .iter()
        .find(|decl| decl.name == struct_name)
        .ok_or_else(|| Diagnostic::backend(format!("unknown struct `{struct_key}`"), 1, 1))
}

fn enum_decl_for_key<'a>(
    unit: &'a CompilationUnit,
    enum_key: &str,
) -> XResult<&'a crate::ast::EnumDecl> {
    let (module_name, enum_name) = enum_key
        .strip_suffix(".tagged")
        .and_then(|prefix| prefix.rsplit_once('.'))
        .ok_or_else(|| Diagnostic::backend(format!("invalid enum key `{enum_key}`"), 1, 1))?;
    let module = unit
        .modules
        .get(module_name)
        .ok_or_else(|| Diagnostic::backend(format!("unknown module `{module_name}`"), 1, 1))?;
    module
        .program
        .enums
        .iter()
        .find(|decl| decl.name == enum_name)
        .ok_or_else(|| Diagnostic::backend(format!("unknown enum `{enum_key}`"), 1, 1))
}

fn variant_tag_index(enum_decl: &crate::ast::EnumDecl, variant: &str) -> XResult<usize> {
    enum_decl
        .variants
        .iter()
        .position(|v| v.name == variant)
        .ok_or_else(|| Diagnostic::backend(format!("unknown variant `{variant}`"), 1, 1))
}

fn variant_payload_type(
    enum_decl: &crate::ast::EnumDecl,
    variant: &str,
) -> XResult<Option<TypeName>> {
    let variant_decl = enum_decl
        .variants
        .iter()
        .find(|v| v.name == variant)
        .ok_or_else(|| Diagnostic::backend(format!("unknown variant `{variant}`"), 1, 1))?;
    Ok(variant_decl
        .payload
        .as_ref()
        .map(|payload| payload.ty.clone()))
}

fn insert_enum_key_if_exists(
    unit: &CompilationUnit,
    module_name: &str,
    ty: &TypeName,
    keys: &mut std::collections::HashSet<String>,
) {
    let Some(key) = enum_key_from_type(module_name, ty) else {
        return;
    };
    if enum_decl_for_key(unit, &key).is_ok() {
        keys.insert(key);
    }
}

fn collect_referenced_enum_types(
    unit: &CompilationUnit,
    program: &Program,
    module_name: &str,
    keys: &mut std::collections::HashSet<String>,
) {
    for function in &program.functions {
        collect_enum_types_in_stmts(unit, &function.body, module_name, keys);
    }
}

fn collect_enum_types_in_stmts(
    unit: &CompilationUnit,
    stmts: &[Stmt],
    module_name: &str,
    keys: &mut std::collections::HashSet<String>,
) {
    for stmt in stmts {
        match stmt {
            Stmt::Let {
                annotation, value, ..
            } => {
                if let Some(ty) = annotation {
                    insert_enum_key_if_exists(unit, module_name, ty, keys);
                }
                collect_enum_types_in_expr(unit, value, module_name, keys);
            }
            Stmt::Assign { value, .. }
            | Stmt::Return {
                value: Some(value), ..
            }
            | Stmt::Expr(value) => collect_enum_types_in_expr(unit, value, module_name, keys),
            Stmt::If {
                condition,
                then_body,
                else_body,
                ..
            } => {
                collect_enum_types_in_expr(unit, condition, module_name, keys);
                collect_enum_types_in_stmts(unit, then_body, module_name, keys);
                collect_enum_types_in_stmts(unit, else_body, module_name, keys);
            }
            Stmt::While {
                condition, body, ..
            } => {
                collect_enum_types_in_expr(unit, condition, module_name, keys);
                collect_enum_types_in_stmts(unit, body, module_name, keys);
            }
            Stmt::AssignIndex { index, value, .. } => {
                collect_enum_types_in_expr(unit, index, module_name, keys);
                collect_enum_types_in_expr(unit, value, module_name, keys);
            }
            Stmt::AssignField { value, .. } => {
                collect_enum_types_in_expr(unit, value, module_name, keys);
            }
            _ => {}
        }
    }
}

fn collect_enum_types_in_expr(
    unit: &CompilationUnit,
    expr: &Expr,
    module_name: &str,
    keys: &mut std::collections::HashSet<String>,
) {
    match expr {
        Expr::Match {
            scrutinee, arms, ..
        } => {
            collect_enum_types_in_expr(unit, scrutinee, module_name, keys);
            for arm in arms {
                match &arm.body {
                    MatchBody::Expr(expr) => {
                        collect_enum_types_in_expr(unit, expr, module_name, keys);
                    }
                    MatchBody::Block(stmts) => {
                        collect_enum_types_in_stmts(unit, stmts, module_name, keys);
                    }
                }
            }
        }
        Expr::Call { args, .. } | Expr::QualifiedCall { args, .. } => {
            for arg in args {
                collect_enum_types_in_expr(unit, arg, module_name, keys);
            }
        }
        Expr::Unary { expr, .. } => collect_enum_types_in_expr(unit, expr, module_name, keys),
        Expr::Binary { left, right, .. } => {
            collect_enum_types_in_expr(unit, left, module_name, keys);
            collect_enum_types_in_expr(unit, right, module_name, keys);
        }
        Expr::ArrayLiteral { elements, .. } | Expr::StructLiteral { elements, .. } => {
            for element in elements {
                collect_enum_types_in_expr(unit, element, module_name, keys);
            }
        }
        Expr::Index { base, index, .. } => {
            collect_enum_types_in_expr(unit, base, module_name, keys);
            collect_enum_types_in_expr(unit, index, module_name, keys);
        }
        Expr::FieldAccess { base, .. } => collect_enum_types_in_expr(unit, base, module_name, keys),
        _ => {}
    }
}

fn collect_referenced_struct_types(
    unit: &CompilationUnit,
    program: &Program,
    module_name: &str,
    keys: &mut std::collections::HashSet<String>,
) {
    for function in &program.functions {
        collect_struct_types_in_stmts(unit, &function.body, module_name, keys);
    }
}

fn insert_struct_key_if_exists(
    unit: &CompilationUnit,
    module_name: &str,
    ty: &TypeName,
    keys: &mut std::collections::HashSet<String>,
) {
    let Some(key) = struct_key_from_type(module_name, ty) else {
        return;
    };
    if struct_decl_for_key(unit, &key).is_ok() {
        keys.insert(key);
    }
}

fn collect_struct_types_in_stmts(
    unit: &CompilationUnit,
    stmts: &[Stmt],
    module_name: &str,
    keys: &mut std::collections::HashSet<String>,
) {
    for stmt in stmts {
        match stmt {
            Stmt::Let {
                annotation, value, ..
            } => {
                if let Some(ty) = annotation {
                    insert_struct_key_if_exists(unit, module_name, ty, keys);
                }
                collect_struct_types_in_expr(unit, value, module_name, keys);
            }
            Stmt::Assign { value, .. }
            | Stmt::Return {
                value: Some(value), ..
            }
            | Stmt::Expr(value) => collect_struct_types_in_expr(unit, value, module_name, keys),
            Stmt::If {
                condition,
                then_body,
                else_body,
                ..
            } => {
                collect_struct_types_in_expr(unit, condition, module_name, keys);
                collect_struct_types_in_stmts(unit, then_body, module_name, keys);
                collect_struct_types_in_stmts(unit, else_body, module_name, keys);
            }
            Stmt::While {
                condition, body, ..
            } => {
                collect_struct_types_in_expr(unit, condition, module_name, keys);
                collect_struct_types_in_stmts(unit, body, module_name, keys);
            }
            Stmt::AssignIndex { index, value, .. } => {
                collect_struct_types_in_expr(unit, index, module_name, keys);
                collect_struct_types_in_expr(unit, value, module_name, keys);
            }
            Stmt::AssignField { value, .. } => {
                collect_struct_types_in_expr(unit, value, module_name, keys);
            }
            _ => {}
        }
    }
}

fn collect_struct_types_in_expr(
    unit: &CompilationUnit,
    expr: &Expr,
    module_name: &str,
    keys: &mut std::collections::HashSet<String>,
) {
    match expr {
        Expr::Call { args, .. } | Expr::QualifiedCall { args, .. } => {
            for arg in args {
                collect_struct_types_in_expr(unit, arg, module_name, keys);
            }
        }
        Expr::Unary { expr, .. } => collect_struct_types_in_expr(unit, expr, module_name, keys),
        Expr::Binary { left, right, .. } => {
            collect_struct_types_in_expr(unit, left, module_name, keys);
            collect_struct_types_in_expr(unit, right, module_name, keys);
        }
        Expr::ArrayLiteral { elements, .. } | Expr::StructLiteral { elements, .. } => {
            for element in elements {
                collect_struct_types_in_expr(unit, element, module_name, keys);
            }
        }
        Expr::Index { base, index, .. } => {
            collect_struct_types_in_expr(unit, base, module_name, keys);
            collect_struct_types_in_expr(unit, index, module_name, keys);
        }
        Expr::FieldAccess { base, .. } => {
            collect_struct_types_in_expr(unit, base, module_name, keys)
        }
        Expr::Match {
            scrutinee, arms, ..
        } => {
            collect_struct_types_in_expr(unit, scrutinee, module_name, keys);
            for arm in arms {
                match &arm.body {
                    MatchBody::Expr(expr) => {
                        collect_struct_types_in_expr(unit, expr, module_name, keys);
                    }
                    MatchBody::Block(stmts) => {
                        collect_struct_types_in_stmts(unit, stmts, module_name, keys);
                    }
                }
            }
        }
        _ => {}
    }
}

fn collect_qualified_calls(
    stmts: &[Stmt],
    needed: &mut std::collections::HashSet<(String, String)>,
) {
    for stmt in stmts {
        match stmt {
            Stmt::Let { value, .. }
            | Stmt::Assign { value, .. }
            | Stmt::Return {
                value: Some(value), ..
            }
            | Stmt::Expr(value) => collect_qualified_calls_in_expr(value, needed),
            Stmt::If {
                condition,
                then_body,
                else_body,
                ..
            } => {
                collect_qualified_calls_in_expr(condition, needed);
                collect_qualified_calls(then_body, needed);
                collect_qualified_calls(else_body, needed);
            }
            Stmt::While {
                condition, body, ..
            } => {
                collect_qualified_calls_in_expr(condition, needed);
                collect_qualified_calls(body, needed);
            }
            Stmt::AssignIndex { index, value, .. } => {
                collect_qualified_calls_in_expr(index, needed);
                collect_qualified_calls_in_expr(value, needed);
            }
            Stmt::AssignField { value, .. } => collect_qualified_calls_in_expr(value, needed),
            _ => {}
        }
    }
}

fn collect_qualified_calls_in_expr(
    expr: &Expr,
    needed: &mut std::collections::HashSet<(String, String)>,
) {
    match expr {
        Expr::QualifiedCall {
            module,
            callee,
            args,
            ..
        } => {
            needed.insert((module.clone(), callee.clone()));
            for arg in args {
                collect_qualified_calls_in_expr(arg, needed);
            }
        }
        Expr::Call { args, .. } => {
            for arg in args {
                collect_qualified_calls_in_expr(arg, needed);
            }
        }
        Expr::Unary { expr, .. } => collect_qualified_calls_in_expr(expr, needed),
        Expr::Binary { left, right, .. } => {
            collect_qualified_calls_in_expr(left, needed);
            collect_qualified_calls_in_expr(right, needed);
        }
        Expr::ArrayLiteral { elements, .. } | Expr::StructLiteral { elements, .. } => {
            for element in elements {
                collect_qualified_calls_in_expr(element, needed);
            }
        }
        Expr::Index { base, index, .. } => {
            collect_qualified_calls_in_expr(base, needed);
            collect_qualified_calls_in_expr(index, needed);
        }
        Expr::FieldAccess { base, .. } => collect_qualified_calls_in_expr(base, needed),
        Expr::Match {
            scrutinee, arms, ..
        } => {
            collect_qualified_calls_in_expr(scrutinee, needed);
            for arm in arms {
                match &arm.body {
                    MatchBody::Expr(expr) => collect_qualified_calls_in_expr(expr, needed),
                    MatchBody::Block(stmts) => collect_qualified_calls(stmts, needed),
                }
            }
        }
        _ => {}
    }
}

fn llvm_scalar_type<'ctx>(
    context: &'ctx Context,
    ty: &TypeName,
    span: crate::diagnostic::Span,
) -> XResult<BasicTypeEnum<'ctx>> {
    match ty {
        TypeName::I32 => Ok(context.i32_type().as_basic_type_enum()),
        TypeName::Bool => Ok(context.bool_type().as_basic_type_enum()),
        _ => Err(unsupported_backend_type(span)),
    }
}

fn ensure_backend_signature_type(
    unit: &CompilationUnit,
    module_name: &str,
    ty: &TypeName,
    span: crate::diagnostic::Span,
) -> XResult<()> {
    match ty {
        TypeName::I32 | TypeName::Bool | TypeName::Void => Ok(()),
        TypeName::Str | TypeName::Array { .. } => Err(unsupported_backend_type(span)),
        TypeName::Named(name) => {
            let enum_key = enum_ir_name(module_name, name);
            if enum_decl_for_key(unit, &enum_key).is_ok() {
                Ok(())
            } else if struct_decl_for_key(unit, &struct_ir_name(module_name, name)).is_ok() {
                Ok(())
            } else {
                Err(Diagnostic::backend_at(
                    format!("unknown type `{name}`"),
                    span,
                ))
            }
        }
        TypeName::Qualified { module, name } => {
            let enum_key = enum_ir_name(module, name);
            if enum_decl_for_key(unit, &enum_key).is_ok() {
                Ok(())
            } else if struct_decl_for_key(unit, &struct_ir_name(module, name)).is_ok() {
                Ok(())
            } else {
                Err(Diagnostic::backend_at(
                    format!("unknown type `{module}.{name}`"),
                    span,
                ))
            }
        }
    }
}

fn ensure_declared_enum_types_for_signature<'ctx>(
    context: &'ctx Context,
    unit: &CompilationUnit,
    module_name: &str,
    enum_types: &mut HashMap<String, StructType<'ctx>>,
    return_type: &TypeName,
    params: &[TypeName],
) -> XResult<()> {
    insert_declared_enum_type(context, unit, module_name, enum_types, return_type)?;
    for param in params {
        insert_declared_enum_type(context, unit, module_name, enum_types, param)?;
    }
    Ok(())
}

fn insert_declared_enum_type<'ctx>(
    context: &'ctx Context,
    unit: &CompilationUnit,
    module_name: &str,
    enum_types: &mut HashMap<String, StructType<'ctx>>,
    ty: &TypeName,
) -> XResult<()> {
    let Some(key) = enum_key_from_type(module_name, ty) else {
        return Ok(());
    };
    if enum_types.contains_key(&key) {
        return Ok(());
    }
    if enum_decl_for_key(unit, &key).is_err() {
        return Ok(());
    }
    let i32 = context.i32_type();
    let enum_type = context
        .get_struct_type(&key)
        .unwrap_or_else(|| context.opaque_struct_type(&key));
    enum_type.set_body(&[i32.into(), i32.into()], false);
    enum_types.insert(key, enum_type);
    Ok(())
}

fn collect_enum_types_in_signatures(
    unit: &CompilationUnit,
    program: &Program,
    module_name: &str,
    keys: &mut std::collections::HashSet<String>,
) {
    for function in &program.functions {
        insert_enum_key_if_exists(unit, module_name, &function.return_type, keys);
        for param in &function.params {
            insert_enum_key_if_exists(unit, module_name, &param.ty, keys);
        }
    }
}

fn ensure_declared_struct_types_for_signature<'ctx>(
    context: &'ctx Context,
    unit: &CompilationUnit,
    module_name: &str,
    struct_types: &mut HashMap<String, StructType<'ctx>>,
    return_type: &TypeName,
    params: &[TypeName],
) -> XResult<()> {
    insert_declared_struct_type(context, unit, module_name, struct_types, return_type)?;
    for param in params {
        insert_declared_struct_type(context, unit, module_name, struct_types, param)?;
    }
    Ok(())
}

fn insert_declared_struct_type<'ctx>(
    context: &'ctx Context,
    unit: &CompilationUnit,
    module_name: &str,
    struct_types: &mut HashMap<String, StructType<'ctx>>,
    ty: &TypeName,
) -> XResult<()> {
    let Some(key) = struct_key_from_type(module_name, ty) else {
        return Ok(());
    };
    if struct_types.contains_key(&key) {
        return Ok(());
    }
    if struct_decl_for_key(unit, &key).is_err() {
        return Ok(());
    }
    let (owner_module, struct_name) = key
        .split_once('.')
        .ok_or_else(|| Diagnostic::backend(format!("invalid struct key `{key}`"), 1, 1))?;
    let owner = unit
        .modules
        .get(owner_module)
        .ok_or_else(|| Diagnostic::backend(format!("unknown module `{owner_module}`"), 1, 1))?;
    let struct_decl = owner
        .program
        .structs
        .iter()
        .find(|decl| decl.name == struct_name)
        .ok_or_else(|| Diagnostic::backend(format!("unknown struct `{key}`"), 1, 1))?;
    let field_types = struct_decl
        .fields
        .iter()
        .map(|field| llvm_scalar_type(context, &field.ty, field.ty_span))
        .collect::<XResult<Vec<BasicTypeEnum<'ctx>>>>()?;
    let struct_type = context
        .get_struct_type(&key)
        .unwrap_or_else(|| context.opaque_struct_type(&key));
    struct_type.set_body(&field_types, false);
    struct_types.insert(key, struct_type);
    Ok(())
}

fn collect_struct_types_in_signatures(
    unit: &CompilationUnit,
    program: &Program,
    module_name: &str,
    keys: &mut std::collections::HashSet<String>,
) {
    for function in &program.functions {
        insert_struct_key_if_exists(unit, module_name, &function.return_type, keys);
        for param in &function.params {
            insert_struct_key_if_exists(unit, module_name, &param.ty, keys);
        }
    }
}

fn llvm_array_type<'ctx>(
    context: &'ctx Context,
    elem: &TypeName,
    len: usize,
) -> XResult<inkwell::types::ArrayType<'ctx>> {
    match elem {
        TypeName::I32 => Ok(context.i32_type().array_type(len as u32)),
        TypeName::Bool => Ok(context.bool_type().array_type(len as u32)),
        _ => Err(unsupported_backend_type(crate::diagnostic::Span::point(
            1, 1,
        ))),
    }
}

fn llvm_basic_type<'ctx>(
    context: &'ctx Context,
    struct_types: &HashMap<String, StructType<'ctx>>,
    enum_types: &HashMap<String, StructType<'ctx>>,
    unit: &CompilationUnit,
    module_name: &str,
    ty: &TypeName,
) -> XResult<BasicTypeEnum<'ctx>> {
    match ty {
        TypeName::I32 => Ok(context.i32_type().as_basic_type_enum()),
        TypeName::Bool => Ok(context.bool_type().as_basic_type_enum()),
        TypeName::Named(_) | TypeName::Qualified { .. } => {
            if let Some(key) = enum_key_from_type(module_name, ty)
                && enum_decl_for_key(unit, &key).is_ok()
            {
                return enum_types
                    .get(&key)
                    .map(|enum_ty| (*enum_ty).as_basic_type_enum())
                    .ok_or_else(|| {
                        Diagnostic::backend(format!("missing enum type `{key}`"), 1, 1)
                    });
            }
            if let Some(key) = struct_key_from_type(module_name, ty)
                && struct_decl_for_key(unit, &key).is_ok()
            {
                return struct_types
                    .get(&key)
                    .map(|struct_ty| (*struct_ty).as_basic_type_enum())
                    .ok_or_else(|| {
                        Diagnostic::backend(format!("missing struct type `{key}`"), 1, 1)
                    });
            }
            Err(unsupported_backend_type(crate::diagnostic::Span::point(
                1, 1,
            )))
        }
        TypeName::Void | TypeName::Str | TypeName::Array { .. } => Err(unsupported_backend_type(
            crate::diagnostic::Span::point(1, 1),
        )),
    }
}

fn expect_int_value(value: Option<BasicValueEnum<'_>>) -> XResult<inkwell::values::IntValue<'_>> {
    let Some(value) = value else {
        return Err(Diagnostic::backend(
            "expected integer value, got void",
            1,
            1,
        ));
    };
    if !value.is_int_value() {
        return Err(unsupported_backend_type(crate::diagnostic::Span::point(
            1, 1,
        )));
    }
    Ok(value.into_int_value())
}

fn build_enum_value<'ctx>(
    builder: &Builder<'ctx>,
    enum_ty: StructType<'ctx>,
    tag: inkwell::values::IntValue<'ctx>,
    payload: inkwell::values::IntValue<'ctx>,
) -> XResult<inkwell::values::StructValue<'ctx>> {
    let undef = enum_ty.get_undef();
    let with_tag =
        build_value(builder.build_insert_value(undef, tag, 0, "enum.tag"))?.into_struct_value();
    Ok(
        build_value(builder.build_insert_value(with_tag, payload, 1, "enum.val"))?
            .into_struct_value(),
    )
}

fn build_value<T>(result: Result<T, inkwell::builder::BuilderError>) -> XResult<T> {
    result.map_err(|err| Diagnostic::backend(format!("LLVM builder error: {err:?}"), 1, 1))
}

fn build_unit<T>(result: Result<T, inkwell::builder::BuilderError>) -> XResult<()> {
    build_value(result).map(|_| ())
}

fn current_block<'ctx>(builder: &Builder<'ctx>) -> XResult<BasicBlock<'ctx>> {
    builder
        .get_insert_block()
        .ok_or_else(|| Diagnostic::backend("LLVM builder is not positioned", 1, 1))
}

fn unsupported_backend_type(span: crate::diagnostic::Span) -> Diagnostic {
    Diagnostic::backend_at(
        "LLVM MVP supports i32, bool, and void code generation only",
        span,
    )
}
