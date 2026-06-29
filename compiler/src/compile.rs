use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::{env, ffi::OsString};

use crate::ast::Program;
use crate::backend::llvm;
use crate::diagnostic::{Diagnostic, XResult};
use crate::{lexer, parser, typeck};

#[derive(Debug, Clone)]
pub struct CheckedProgram {
    pub program: Program,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CompileOptions {
    pub target_triple: Option<String>,
}

impl CompileOptions {
    pub fn from_env() -> Self {
        Self {
            target_triple: env_string("XLANG_TARGET_TRIPLE"),
        }
    }
}

pub fn check(file: &Path) -> XResult<CheckedProgram> {
    let source = fs::read_to_string(file)
        .map_err(|err| Diagnostic::new(format!("failed to read source: {err}"), 1, 1))?;
    check_source(&source)
}

pub fn check_source(source: &str) -> XResult<CheckedProgram> {
    let tokens = lexer::lex(source)?;
    let program = parser::parse(tokens)?;
    typeck::check(&program)?;
    Ok(CheckedProgram { program })
}

pub fn emit_llvm(file: &Path) -> XResult<String> {
    emit_llvm_with_options(file, &CompileOptions::from_env())
}

pub fn emit_llvm_with_options(file: &Path, options: &CompileOptions) -> XResult<String> {
    let checked = check(file)?;
    llvm::emit_llvm_ir_with_options(&checked.program, &llvm_options(options))
}

pub fn emit_llvm_source(source: &str) -> XResult<String> {
    emit_llvm_source_with_options(source, &CompileOptions::from_env())
}

pub fn emit_llvm_source_with_options(source: &str, options: &CompileOptions) -> XResult<String> {
    let checked = check_source(source)?;
    llvm::emit_llvm_ir_with_options(&checked.program, &llvm_options(options))
}

pub fn build(file: &Path) -> XResult<PathBuf> {
    build_with_options(file, &CompileOptions::from_env())
}

pub fn build_with_options(file: &Path, options: &CompileOptions) -> XResult<PathBuf> {
    let llvm_ir = emit_llvm_with_options(file, options)?;
    let build_dir = PathBuf::from("build");
    fs::create_dir_all(&build_dir)
        .map_err(|err| Diagnostic::new(format!("failed to create build directory: {err}"), 1, 1))?;
    let ir_path = build_dir.join("main.ll");
    fs::write(&ir_path, llvm_ir)
        .map_err(|err| Diagnostic::new(format!("failed to write LLVM IR: {err}"), 1, 1))?;

    let executable = build_dir.join(if cfg!(windows) { "main.exe" } else { "main" });
    if command_exists("clang") {
        let mut clang = Command::new("clang");
        clang.arg("-Wno-override-module");
        if let Some(target_triple) = &options.target_triple {
            clang.arg("-target").arg(target_triple);
        }
        let status = clang
            .arg(&ir_path)
            .arg("-o")
            .arg(&executable)
            .status()
            .map_err(|err| Diagnostic::new(format!("failed to invoke clang: {err}"), 1, 1))?;
        if status.success() {
            return Ok(executable);
        }
        return Err(Diagnostic::new("LLVM native build failed via clang", 1, 1));
    }

    Err(Diagnostic::new(
        format!(
            "wrote LLVM IR to {}; install clang or another LLVM toolchain on PATH to produce a native executable",
            ir_path.display()
        ),
        1,
        1,
    ))
}

pub fn run(file: &Path) -> XResult<i32> {
    run_with_options(file, &CompileOptions::from_env())
}

pub fn run_with_options(file: &Path, options: &CompileOptions) -> XResult<i32> {
    let executable = build_with_options(file, options)?;
    let status = Command::new(&executable)
        .status()
        .map_err(|err| Diagnostic::new(format!("failed to run executable: {err}"), 1, 1))?;
    Ok(status.code().unwrap_or(1))
}

fn llvm_options(options: &CompileOptions) -> llvm::LlvmOptions {
    llvm::LlvmOptions {
        target_triple: options.target_triple.clone(),
    }
}

fn env_string(key: &str) -> Option<String> {
    env::var_os(key).and_then(non_empty_string)
}

fn non_empty_string(value: OsString) -> Option<String> {
    let value = value.to_string_lossy().trim().to_owned();
    (!value.is_empty()).then_some(value)
}

fn command_exists(command: &str) -> bool {
    Command::new(command)
        .arg("--version")
        .output()
        .map(|output| output.status.success())
        .unwrap_or(false)
}
