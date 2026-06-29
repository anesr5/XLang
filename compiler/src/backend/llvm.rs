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

use crate::ast::{BinaryOp, Expr, Function, Program, Stmt, TypeName, UnaryOp};
use crate::diagnostic::{Diagnostic, XResult};

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct LlvmOptions {
    pub target_triple: Option<String>,
}

pub fn emit_llvm_ir(program: &Program) -> XResult<String> {
    emit_llvm_ir_with_options(program, &LlvmOptions::default())
}

pub fn emit_llvm_ir_with_options(program: &Program, options: &LlvmOptions) -> XResult<String> {
    let context = Context::create();
    let mut emitter = LlvmEmitter::new(&context, program, options.target_triple.as_deref());
    emitter.emit_program()
}

struct LlvmEmitter<'ctx, 'program> {
    context: &'ctx Context,
    module: Module<'ctx>,
    builder: Builder<'ctx>,
    program: &'program Program,
    functions: HashMap<&'program str, FunctionValue<'ctx>>,
    struct_types: HashMap<String, StructType<'ctx>>,
}

struct FunctionEmitter<'emit, 'ctx, 'program> {
    context: &'ctx Context,
    builder: &'emit Builder<'ctx>,
    function: FunctionValue<'ctx>,
    functions: &'emit HashMap<&'emit str, FunctionValue<'ctx>>,
    module: &'emit Module<'ctx>,
    program: &'program Program,
    struct_types: &'emit HashMap<String, StructType<'ctx>>,
    locals: HashMap<String, Local<'ctx>>,
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

impl<'ctx, 'program> LlvmEmitter<'ctx, 'program> {
    fn new(
        context: &'ctx Context,
        program: &'program Program,
        target_triple: Option<&str>,
    ) -> Self {
        let module = context.create_module("xlang");
        if let Some(triple) = target_triple {
            module.set_triple(&TargetTriple::create(triple));
        } else if let Some(triple) = host_target_triple() {
            module.set_triple(&TargetTriple::create(triple));
        }
        Self {
            context,
            module,
            builder: context.create_builder(),
            program,
            functions: HashMap::new(),
            struct_types: HashMap::new(),
        }
    }

    fn emit_program(&mut self) -> XResult<String> {
        self.ensure_supported()?;
        self.declare_struct_types()?;
        self.declare_functions()?;
        for function in &self.program.functions {
            self.emit_function(function)?;
        }
        self.module.verify().map_err(|message| {
            Diagnostic::backend(format!("LLVM verifier failed: {message}"), 1, 1)
        })?;
        Ok(self.module.print_to_string().to_string())
    }

    fn ensure_supported(&self) -> XResult<()> {
        for struct_decl in &self.program.structs {
            for field in &struct_decl.fields {
                ensure_struct_field_backend_type(&field.ty, field.ty_span)?;
            }
        }
        for function in &self.program.functions {
            ensure_backend_type(
                &function.return_type,
                function.return_type_span.unwrap_or(function.name_span),
            )?;
            for param in &function.params {
                ensure_backend_type(&param.ty, param.ty_span)?;
            }
            for stmt in &function.body {
                ensure_stmt_supported(self.program, stmt)?;
            }
        }
        Ok(())
    }

    fn declare_struct_types(&mut self) -> XResult<()> {
        for struct_decl in &self.program.structs {
            let field_types = struct_decl
                .fields
                .iter()
                .map(|field| llvm_scalar_type(self.context, &field.ty, field.ty_span))
                .collect::<XResult<Vec<BasicTypeEnum<'ctx>>>>()?;
            let struct_type = self
                .context
                .get_struct_type(&struct_decl.name)
                .unwrap_or_else(|| self.context.opaque_struct_type(&struct_decl.name));
            struct_type.set_body(&field_types, false);
            self.struct_types
                .insert(struct_decl.name.clone(), struct_type);
        }
        Ok(())
    }

    fn declare_functions(&mut self) -> XResult<()> {
        for function in &self.program.functions {
            let params = function
                .params
                .iter()
                .map(|param| llvm_basic_type(self.context, &param.ty).map(Into::into))
                .collect::<XResult<Vec<BasicMetadataTypeEnum<'ctx>>>>()?;
            let function_type = match function.return_type {
                TypeName::Void => self.context.void_type().fn_type(&params, false),
                _ => llvm_basic_type(self.context, &function.return_type)?.fn_type(&params, false),
            };
            let function_value = self
                .module
                .add_function(&function.name, function_type, None);
            self.functions
                .insert(function.name.as_str(), function_value);
        }
        Ok(())
    }

    fn emit_function(&self, function: &Function) -> XResult<()> {
        let function_value = self
            .functions
            .get(function.name.as_str())
            .copied()
            .ok_or_else(|| {
                Diagnostic::backend(format!("unknown function `{}`", function.name), 1, 1)
            })?;
        let entry = self.context.append_basic_block(function_value, "entry");
        self.builder.position_at_end(entry);

        let mut emitter = FunctionEmitter {
            context: self.context,
            builder: &self.builder,
            function: function_value,
            functions: &self.functions,
            module: &self.module,
            program: self.program,
            struct_types: &self.struct_types,
            locals: HashMap::new(),
            loop_stack: Vec::new(),
            terminated: false,
        };

        for (index, param) in function.params.iter().enumerate() {
            let value = function_value
                .get_nth_param(index as u32)
                .ok_or_else(|| Diagnostic::backend("missing LLVM function parameter", 1, 1))?;
            value.set_name(&param.name);
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

impl<'emit, 'ctx, 'program> FunctionEmitter<'emit, 'ctx, 'program> {
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
                if let Some(TypeName::Array { elem, len }) = annotation {
                    self.emit_array_binding(name, elem, *len, value, source_span)?;
                } else if let Some(TypeName::Named(struct_name)) = annotation {
                    self.emit_struct_binding(name, struct_name, value, source_span)?;
                } else {
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
                if matches!(local.ty, TypeName::Named(_)) {
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
                let (struct_ptr, struct_name) = {
                    let local = self.locals.get(name).ok_or_else(|| {
                        Diagnostic::backend(format!("unknown variable `{name}`"), 1, 1)
                    })?;
                    let TypeName::Named(struct_name) = &local.ty else {
                        return Err(Diagnostic::backend_at(
                            "field assignment requires struct binding",
                            value.span(),
                        ));
                    };
                    (local.pointer, struct_name.clone())
                };
                let field_index = struct_field_index(self.program, &struct_name, field)?;
                let field_ty = struct_field_type(self.program, &struct_name, field_index)?;
                let stored = self.emit_expr(value)?;
                let Some(stored) = stored else {
                    return Err(Diagnostic::backend_at(
                        "cannot assign void value",
                        value.span(),
                    ));
                };
                let field_ptr =
                    self.emit_struct_field_ptr(struct_ptr, &struct_name, field_index)?;
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
        struct_name: &str,
        value: &Expr,
        source_span: crate::diagnostic::Span,
    ) -> XResult<()> {
        let struct_ty = self.struct_types.get(struct_name).ok_or_else(|| {
            Diagnostic::backend_at(format!("unknown struct type `{struct_name}`"), source_span)
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
            let field_ptr = self.emit_struct_field_ptr(slot, struct_name, index)?;
            build_unit(self.builder.build_store(field_ptr, rendered))?;
        }
        self.locals.insert(
            name.to_owned(),
            Local {
                pointer: slot,
                ty: TypeName::Named(struct_name.to_owned()),
                scalar_ty: None,
            },
        );
        Ok(())
    }

    fn emit_struct_field_ptr(
        &mut self,
        struct_ptr: PointerValue<'ctx>,
        struct_name: &str,
        field_index: usize,
    ) -> XResult<PointerValue<'ctx>> {
        let struct_ty = self.struct_types.get(struct_name).ok_or_else(|| {
            Diagnostic::backend(format!("unknown struct type `{struct_name}`"), 1, 1)
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
                let Some(scalar_ty) = local.scalar_ty else {
                    if local.ty.array_elem_len().is_some() {
                        return Err(Diagnostic::backend_at(
                            "cannot use array binding as scalar value",
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
            Expr::Call { callee, args, .. } => self.emit_call(callee, args),
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
                let scalar_ty = llvm_basic_type(self.context, &elem_ty)?;
                Ok(Some(build_value(self.builder.build_load(
                    scalar_ty,
                    elem_ptr,
                    "index.load",
                ))?))
            }
            Expr::StructLiteral { span, .. } => Err(Diagnostic::backend_at(
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
                    return Err(Diagnostic::backend_at(
                        "field access base must be a variable",
                        base.span(),
                    ));
                };
                let (struct_ptr, struct_name) = {
                    let local = self.locals.get(name).ok_or_else(|| {
                        Diagnostic::backend_at(format!("unknown variable `{name}`"), *base_span)
                    })?;
                    let TypeName::Named(struct_name) = &local.ty else {
                        return Err(Diagnostic::backend_at(
                            "field access requires struct binding",
                            *field_span,
                        ));
                    };
                    (local.pointer, struct_name.clone())
                };
                let field_index = struct_field_index(self.program, &struct_name, field)?;
                let field_ty = struct_field_type(self.program, &struct_name, field_index)?;
                let field_ptr =
                    self.emit_struct_field_ptr(struct_ptr, &struct_name, field_index)?;
                let scalar_ty = llvm_scalar_type(self.context, &field_ty, *field_span)?;
                Ok(Some(build_value(self.builder.build_load(
                    scalar_ty,
                    field_ptr,
                    "field.load",
                ))?))
            }
        }
    }

    fn emit_call(&mut self, callee: &str, args: &[Expr]) -> XResult<Option<BasicValueEnum<'ctx>>> {
        let function = self
            .functions
            .get(callee)
            .copied()
            .ok_or_else(|| Diagnostic::backend(format!("unknown function `{callee}`"), 1, 1))?;
        let mut rendered = Vec::new();
        for arg in args {
            let value = self.emit_expr(arg)?;
            let Some(value) = value else {
                return Err(Diagnostic::backend("cannot pass void as an argument", 1, 1));
            };
            rendered.push(BasicMetadataValueEnum::from(value));
        }
        let call = build_value(self.builder.build_call(function, &rendered, "calltmp"))?;
        let return_type = function_return_type(function)?;
        if return_type == TypeName::Void {
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

fn ensure_stmt_supported(program: &Program, stmt: &Stmt) -> XResult<()> {
    match stmt {
        Stmt::Let {
            value, annotation, ..
        } => {
            if let Some(ty) = annotation {
                ensure_local_type_supported(program, ty, value.span())?;
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
                ensure_stmt_supported(program, stmt)?;
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
                ensure_stmt_supported(program, stmt)?;
            }
            Ok(())
        }
    }
}

fn ensure_expr_supported(expr: &Expr) -> XResult<()> {
    match expr {
        Expr::String { span, .. } => Err(unsupported_backend_type(*span)),
        Expr::Call { args, .. } => {
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
    }
}

fn ensure_struct_field_backend_type(ty: &TypeName, span: crate::diagnostic::Span) -> XResult<()> {
    match ty {
        TypeName::I32 | TypeName::Bool => Ok(()),
        TypeName::Str => Err(Diagnostic::backend_at(
            "LLVM backend does not support struct field type str",
            span,
        )),
        TypeName::Named(_) => Err(Diagnostic::backend_at(
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
        TypeName::Str | TypeName::Named(_) | TypeName::Void | TypeName::Array { .. } => {
            Err(unsupported_backend_type(span))
        }
    }
}

fn ensure_local_type_supported(
    program: &Program,
    ty: &TypeName,
    span: crate::diagnostic::Span,
) -> XResult<()> {
    match ty {
        TypeName::I32 | TypeName::Bool | TypeName::Void => Ok(()),
        TypeName::Array { elem, .. } => ensure_array_element_backend_type(elem, span),
        TypeName::Named(name) => {
            let Some(struct_decl) = program.structs.iter().find(|s| s.name == *name) else {
                return Err(Diagnostic::backend_at(
                    format!("unknown struct type `{name}`"),
                    span,
                ));
            };
            for field in &struct_decl.fields {
                ensure_struct_field_backend_type(&field.ty, field.ty_span)?;
            }
            Ok(())
        }
        TypeName::Str => Err(unsupported_backend_type(span)),
    }
}

fn struct_field_index(program: &Program, struct_name: &str, field: &str) -> XResult<usize> {
    let struct_decl = program
        .structs
        .iter()
        .find(|s| s.name == struct_name)
        .ok_or_else(|| Diagnostic::backend(format!("unknown struct `{struct_name}`"), 1, 1))?;
    struct_decl
        .fields
        .iter()
        .position(|f| f.name == field)
        .ok_or_else(|| {
            Diagnostic::backend(
                format!("struct `{struct_name}` has no field `{field}`"),
                1,
                1,
            )
        })
}

fn struct_field_type(
    program: &Program,
    struct_name: &str,
    field_index: usize,
) -> XResult<TypeName> {
    let struct_decl = program
        .structs
        .iter()
        .find(|s| s.name == struct_name)
        .ok_or_else(|| Diagnostic::backend(format!("unknown struct `{struct_name}`"), 1, 1))?;
    struct_decl
        .fields
        .get(field_index)
        .map(|field| field.ty.clone())
        .ok_or_else(|| {
            Diagnostic::backend(format!("invalid field index for `{struct_name}`"), 1, 1)
        })
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

fn ensure_backend_type(ty: &TypeName, span: crate::diagnostic::Span) -> XResult<()> {
    match ty {
        TypeName::I32 | TypeName::Bool | TypeName::Void => Ok(()),
        TypeName::Array { .. } | TypeName::Str | TypeName::Named(_) => {
            Err(unsupported_backend_type(span))
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

fn llvm_basic_type<'ctx>(context: &'ctx Context, ty: &TypeName) -> XResult<BasicTypeEnum<'ctx>> {
    match ty {
        TypeName::I32 => Ok(context.i32_type().as_basic_type_enum()),
        TypeName::Bool => Ok(context.bool_type().as_basic_type_enum()),
        TypeName::Void | TypeName::Str | TypeName::Named(_) | TypeName::Array { .. } => Err(
            unsupported_backend_type(crate::diagnostic::Span::point(1, 1)),
        ),
    }
}

fn function_return_type(function: FunctionValue<'_>) -> XResult<TypeName> {
    let Some(return_type) = function.get_type().get_return_type() else {
        return Ok(TypeName::Void);
    };
    if return_type.is_int_type() {
        let width = return_type.into_int_type().get_bit_width();
        return match width {
            1 => Ok(TypeName::Bool),
            32 => Ok(TypeName::I32),
            _ => Err(Diagnostic::backend(
                format!("unsupported LLVM integer return width `{width}`"),
                1,
                1,
            )),
        };
    }
    Err(unsupported_backend_type(crate::diagnostic::Span::point(
        1, 1,
    )))
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
