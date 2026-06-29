use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::{env, ffi::OsString};

use crate::backend::llvm;
use crate::diagnostic::{Diagnostic, XResult};
use crate::modules::CompilationUnit;
use crate::typeck;

#[derive(Debug, Clone)]
pub struct CheckedProgram {
    pub unit: CompilationUnit,
}

impl CheckedProgram {
    pub fn program(&self) -> &crate::ast::Program {
        &self.unit.entry_module().program
    }
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
    let unit = CompilationUnit::load(file)?;
    typeck::check_unit(&unit)?;
    Ok(CheckedProgram { unit })
}

pub fn check_source(source: &str) -> XResult<CheckedProgram> {
    let unit = CompilationUnit::from_source(source)?;
    typeck::check_unit(&unit)?;
    Ok(CheckedProgram { unit })
}

pub fn emit_llvm(file: &Path) -> XResult<String> {
    emit_llvm_with_options(file, &CompileOptions::from_env())
}

pub fn emit_llvm_with_options(file: &Path, options: &CompileOptions) -> XResult<String> {
    let checked = check(file)?;
    let mut modules = llvm::emit_compilation_unit(&checked.unit, &llvm_options(options))?;
    let entry = checked.unit.entry.clone();
    modules.remove(&entry).ok_or_else(|| {
        Diagnostic::backend(format!("missing LLVM IR for entry module `{entry}`"), 1, 1)
    })
}

pub fn emit_llvm_source(source: &str) -> XResult<String> {
    emit_llvm_source_with_options(source, &CompileOptions::from_env())
}

pub fn emit_llvm_source_with_options(source: &str, options: &CompileOptions) -> XResult<String> {
    let checked = check_source(source)?;
    let mut modules = llvm::emit_compilation_unit(&checked.unit, &llvm_options(options))?;
    let entry = checked.unit.entry.clone();
    modules.remove(&entry).ok_or_else(|| {
        Diagnostic::backend(format!("missing LLVM IR for entry module `{entry}`"), 1, 1)
    })
}

pub fn build(file: &Path) -> XResult<PathBuf> {
    build_with_options(file, &CompileOptions::from_env())
}

pub fn build_with_options(file: &Path, options: &CompileOptions) -> XResult<PathBuf> {
    let checked = check(file)?;
    let modules = llvm::emit_compilation_unit(&checked.unit, &llvm_options(options))?;
    let build_dir = PathBuf::from("build");
    fs::create_dir_all(&build_dir)
        .map_err(|err| Diagnostic::io(format!("failed to create build directory: {err}"), 1, 1))?;

    let mut ir_paths = Vec::new();
    for (module_name, ir) in &modules {
        let ir_path = build_dir.join(format!("{module_name}.ll"));
        fs::write(&ir_path, ir).map_err(|err| {
            Diagnostic::io(format!("failed to write LLVM IR for `{module_name}`: {err}"), 1, 1)
        })?;
        ir_paths.push(ir_path);
    }

    let executable = build_dir.join(if cfg!(windows) { "main.exe" } else { "main" });
    if command_exists("clang") {
        let mut clang = Command::new("clang");
        clang.arg("-Wno-override-module");
        if let Some(target_triple) = &options.target_triple {
            clang.arg("-target").arg(target_triple);
        }
        for ir_path in &ir_paths {
            clang.arg(ir_path);
        }
        let status = clang
            .arg("-o")
            .arg(&executable)
            .status()
            .map_err(|err| Diagnostic::io(format!("failed to invoke clang: {err}"), 1, 1))?;
        if status.success() {
            return Ok(executable);
        }
        return Err(Diagnostic::backend(
            "LLVM native build failed via clang",
            1,
            1,
        ));
    }

    Err(Diagnostic::backend(
        format!(
            "wrote LLVM IR to build/; install clang or another LLVM toolchain on PATH to produce a native executable"
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
        .map_err(|err| Diagnostic::io(format!("failed to run executable: {err}"), 1, 1))?;
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
