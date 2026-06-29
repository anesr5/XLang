use std::collections::HashMap;

use inkwell::IntPredicate;
use inkwell::basic_block::BasicBlock;
use inkwell::builder::Builder;
use inkwell::context::Context;
use inkwell::module::Module;
use inkwell::targets::TargetTriple;
use inkwell::types::{BasicMetadataTypeEnum, BasicType, BasicTypeEnum};
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
}

struct FunctionEmitter<'emit, 'ctx> {
    context: &'ctx Context,
    builder: &'emit Builder<'ctx>,
    function: FunctionValue<'ctx>,
    functions: &'emit HashMap<&'emit str, FunctionValue<'ctx>>,
    locals: HashMap<String, Local<'ctx>>,
    terminated: bool,
}

#[derive(Clone)]
struct Local<'ctx> {
    pointer: PointerValue<'ctx>,
    ty: BasicTypeEnum<'ctx>,
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
        }
    }

    fn emit_program(&mut self) -> XResult<String> {
        self.ensure_supported()?;
        self.declare_functions()?;
        for function in &self.program.functions {
            self.emit_function(function)?;
        }
        self.module
            .verify()
            .map_err(|message| Diagnostic::new(format!("LLVM verifier failed: {message}"), 1, 1))?;
        Ok(self.module.print_to_string().to_string())
    }

    fn ensure_supported(&self) -> XResult<()> {
        for function in &self.program.functions {
            ensure_backend_type(
                &function.return_type,
                function.return_type_span.unwrap_or(function.name_span),
            )?;
            for param in &function.params {
                ensure_backend_type(&param.ty, param.ty_span)?;
            }
            for stmt in &function.body {
                ensure_stmt_supported(stmt)?;
            }
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
                Diagnostic::new(format!("unknown function `{}`", function.name), 1, 1)
            })?;
        let entry = self.context.append_basic_block(function_value, "entry");
        self.builder.position_at_end(entry);

        let mut emitter = FunctionEmitter {
            context: self.context,
            builder: &self.builder,
            function: function_value,
            functions: &self.functions,
            locals: HashMap::new(),
            terminated: false,
        };

        for (index, param) in function.params.iter().enumerate() {
            let value = function_value
                .get_nth_param(index as u32)
                .ok_or_else(|| Diagnostic::new("missing LLVM function parameter", 1, 1))?;
            value.set_name(&param.name);
            let ty = value.get_type();
            let slot = build_value(
                self.builder
                    .build_alloca(ty, &format!("{}.addr", param.name)),
            )?;
            build_unit(self.builder.build_store(slot, value))?;
            emitter
                .locals
                .insert(param.name.clone(), Local { pointer: slot, ty });
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

impl<'emit, 'ctx> FunctionEmitter<'emit, 'ctx> {
    fn emit_stmt(&mut self, stmt: &Stmt) -> XResult<()> {
        if self.terminated {
            return Ok(());
        }
        match stmt {
            Stmt::Let { name, value, .. } => {
                let source_span = value.span();
                let value = self.emit_expr(value)?;
                let Some(value) = value else {
                    return Err(Diagnostic::at("cannot bind void value", source_span));
                };
                let ty = value.get_type();
                let slot = build_value(self.builder.build_alloca(ty, name.as_str()))?;
                build_unit(self.builder.build_store(slot, value))?;
                self.locals
                    .insert(name.clone(), Local { pointer: slot, ty });
            }
            Stmt::Assign { name, value, .. } => {
                let source_span = value.span();
                let value = self.emit_expr(value)?;
                let Some(value) = value else {
                    return Err(Diagnostic::at("cannot assign void value", source_span));
                };
                let local = self
                    .locals
                    .get(name)
                    .ok_or_else(|| Diagnostic::new(format!("unknown variable `{name}`"), 1, 1))?;
                build_unit(self.builder.build_store(local.pointer, value))?;
            }
            Stmt::Return {
                value: Some(expr), ..
            } => {
                let value = self.emit_expr(expr)?;
                let Some(value) = value else {
                    return Err(Diagnostic::at("cannot return void value", expr.span()));
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
        }
        Ok(())
    }

    fn emit_if(&mut self, condition: &Expr, then_body: &[Stmt], else_body: &[Stmt]) -> XResult<()> {
        let condition_span = condition.span();
        let condition = self.emit_expr(condition)?;
        let condition = condition
            .ok_or_else(|| Diagnostic::at("if condition cannot be void", condition_span))?
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
            self.terminated = true;
        } else {
            self.builder.position_at_end(end_block);
            self.terminated = false;
        }
        Ok(())
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
            Expr::Variable { name, .. } => {
                let local = self
                    .locals
                    .get(name)
                    .ok_or_else(|| Diagnostic::new(format!("unknown variable `{name}`"), 1, 1))?;
                Ok(Some(build_value(self.builder.build_load(
                    local.ty,
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
        }
    }

    fn emit_call(&mut self, callee: &str, args: &[Expr]) -> XResult<Option<BasicValueEnum<'ctx>>> {
        let function = self
            .functions
            .get(callee)
            .copied()
            .ok_or_else(|| Diagnostic::new(format!("unknown function `{callee}`"), 1, 1))?;
        let mut rendered = Vec::new();
        for arg in args {
            let value = self.emit_expr(arg)?;
            let Some(value) = value else {
                return Err(Diagnostic::new("cannot pass void as an argument", 1, 1));
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
            .ok_or_else(|| Diagnostic::new("LLVM builder is not positioned", 1, 1))?;
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
            .ok_or_else(|| Diagnostic::new("LLVM builder is not positioned", 1, 1))?;
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

fn ensure_stmt_supported(stmt: &Stmt) -> XResult<()> {
    match stmt {
        Stmt::Let { value, .. }
        | Stmt::Assign { value, .. }
        | Stmt::Return {
            value: Some(value), ..
        }
        | Stmt::Expr(value) => ensure_expr_supported(value),
        Stmt::Return { value: None, .. } => Ok(()),
        Stmt::If {
            condition,
            keyword_span: _,
            then_body,
            else_body,
        } => {
            ensure_expr_supported(condition)?;
            for stmt in then_body.iter().chain(else_body.iter()) {
                ensure_stmt_supported(stmt)?;
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
    }
}

fn ensure_backend_type(ty: &TypeName, span: crate::diagnostic::Span) -> XResult<()> {
    match ty {
        TypeName::I32 | TypeName::Bool | TypeName::Void => Ok(()),
        TypeName::Str | TypeName::Named(_) => Err(unsupported_backend_type(span)),
    }
}

fn llvm_basic_type<'ctx>(context: &'ctx Context, ty: &TypeName) -> XResult<BasicTypeEnum<'ctx>> {
    match ty {
        TypeName::I32 => Ok(context.i32_type().as_basic_type_enum()),
        TypeName::Bool => Ok(context.bool_type().as_basic_type_enum()),
        TypeName::Void | TypeName::Str | TypeName::Named(_) => Err(unsupported_backend_type(
            crate::diagnostic::Span::point(1, 1),
        )),
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
            _ => Err(Diagnostic::new(
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
        return Err(Diagnostic::new("expected integer value, got void", 1, 1));
    };
    if !value.is_int_value() {
        return Err(unsupported_backend_type(crate::diagnostic::Span::point(
            1, 1,
        )));
    }
    Ok(value.into_int_value())
}

fn build_value<T>(result: Result<T, inkwell::builder::BuilderError>) -> XResult<T> {
    result.map_err(|err| Diagnostic::new(format!("LLVM builder error: {err:?}"), 1, 1))
}

fn build_unit<T>(result: Result<T, inkwell::builder::BuilderError>) -> XResult<()> {
    build_value(result).map(|_| ())
}

fn current_block<'ctx>(builder: &Builder<'ctx>) -> XResult<BasicBlock<'ctx>> {
    builder
        .get_insert_block()
        .ok_or_else(|| Diagnostic::new("LLVM builder is not positioned", 1, 1))
}

fn unsupported_backend_type(span: crate::diagnostic::Span) -> Diagnostic {
    Diagnostic::at(
        "LLVM MVP supports i32, bool, and void code generation only",
        span,
    )
}
