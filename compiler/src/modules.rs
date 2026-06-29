use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};

use crate::ast::Program;
use crate::diagnostic::{Diagnostic, Span, XResult};
use crate::{lexer, parser};

#[derive(Debug, Clone)]
pub struct LoadedModule {
    pub name: String,
    pub path: Option<PathBuf>,
    pub program: Program,
    /// When true, all top-level items are treated as exported (single-file compat).
    pub implicit_pub: bool,
}

#[derive(Debug, Clone)]
pub struct CompilationUnit {
    pub entry: String,
    pub project_root: Option<PathBuf>,
    pub modules: HashMap<String, LoadedModule>,
}

impl CompilationUnit {
    pub fn from_program(program: Program) -> XResult<Self> {
        validate_imports(&program)?;
        let name = program.module.clone().unwrap_or_else(|| "main".to_owned());
        let implicit_pub = program.imports.is_empty();
        let module = LoadedModule {
            name: name.clone(),
            path: None,
            program,
            implicit_pub,
        };
        let mut modules = HashMap::new();
        modules.insert(name.clone(), module);
        Ok(Self {
            entry: name,
            project_root: None,
            modules,
        })
    }

    pub fn from_source(source: &str) -> XResult<Self> {
        let tokens = lexer::lex(source)?;
        let program = parser::parse(tokens)?;
        Self::from_program(program)
    }

    pub fn load(entry: &Path) -> XResult<Self> {
        let entry = entry
            .canonicalize()
            .map_err(|err| Diagnostic::io(format!("failed to resolve entry path: {err}"), 1, 1))?;
        let project_root = entry
            .parent()
            .map(Path::to_path_buf)
            .ok_or_else(|| Diagnostic::io("entry file has no parent directory", 1, 1))?;

        let mut unit = Self {
            entry: String::new(),
            project_root: Some(project_root.clone()),
            modules: HashMap::new(),
        };

        let entry_name = load_module_file(&mut unit, &entry, &project_root, &mut Vec::new(), true)?;
        unit.entry = entry_name;
        Ok(unit)
    }

    pub fn entry_module(&self) -> &LoadedModule {
        self.modules
            .get(&self.entry)
            .expect("entry module must exist")
    }

    pub fn is_single_module(&self) -> bool {
        self.modules.len() == 1
    }
}

fn load_module_file(
    unit: &mut CompilationUnit,
    path: &Path,
    project_root: &Path,
    stack: &mut Vec<String>,
    is_entry: bool,
) -> XResult<String> {
    let source = fs::read_to_string(path).map_err(|err| {
        Diagnostic::io(format!("failed to read `{}`: {err}", path.display()), 1, 1)
    })?;
    let tokens = lexer::lex(&source)?;
    let program = parser::parse(tokens)?;
    validate_imports(&program)?;

    let expected = expected_module_name_from_path(path, project_root)?;
    let declared = program.module.as_deref().ok_or_else(|| {
        Diagnostic::type_error("expected `module` declaration at top of file", 1, 1)
    })?;
    if declared != expected {
        return Err(Diagnostic::type_error(
            format!(
                "module name `{declared}` does not match file `{}` (expected `{expected}`)",
                path.file_name().and_then(|n| n.to_str()).unwrap_or("?")
            ),
            1,
            1,
        ));
    }

    if unit.modules.contains_key(declared) {
        return Err(Diagnostic::type_error(
            format!("duplicate module `{declared}`"),
            1,
            1,
        ));
    }

    if stack.contains(&declared.to_owned()) {
        let cycle = format_cycle(stack, declared);
        return Err(Diagnostic::type_error(
            format!("circular import: {cycle}"),
            1,
            1,
        ));
    }

    let implicit_pub = false;
    unit.modules.insert(
        declared.to_owned(),
        LoadedModule {
            name: declared.to_owned(),
            path: Some(path.to_path_buf()),
            program: program.clone(),
            implicit_pub,
        },
    );

    stack.push(declared.to_owned());
    let imports: Vec<(String, Span)> = program
        .imports
        .iter()
        .map(|import| (import.name.clone(), import.span))
        .collect();

    for (import_name, import_span) in imports {
        if import_name == declared {
            return Err(Diagnostic::type_error_at(
                format!("module `{declared}` cannot import itself"),
                import_span,
            ));
        }
        if unit.modules.contains_key(&import_name) {
            if stack.contains(&import_name) {
                let cycle = format_cycle(stack, &import_name);
                return Err(Diagnostic::type_error_at(
                    format!("circular import: {cycle}"),
                    import_span,
                ));
            }
            continue;
        }
        if stack.contains(&import_name) {
            let cycle = format_cycle(stack, &import_name);
            return Err(Diagnostic::type_error_at(
                format!("circular import: {cycle}"),
                import_span,
            ));
        }
        let Some(import_path) = resolve_module_path(project_root, &import_name)? else {
            return Err(Diagnostic::type_error_at(
                format!("module `{import_name}` not found"),
                import_span,
            ));
        };
        load_module_file(unit, &import_path, project_root, stack, false)?;
    }
    stack.pop();

    if is_entry {
        Ok(declared.to_owned())
    } else {
        Ok(declared.to_owned())
    }
}

fn validate_imports(program: &Program) -> XResult<()> {
    let mut seen = HashSet::new();
    for import in &program.imports {
        if !seen.insert(import.name.clone()) {
            return Err(Diagnostic::type_error_at(
                format!("duplicate import `{}`", import.name),
                import.span,
            ));
        }
    }
    Ok(())
}

fn resolve_module_path(root: &Path, name: &str) -> XResult<Option<PathBuf>> {
    let flat = root.join(format!("{name}.x"));
    let nested = root.join(name).join("main.x");
    let flat_exists = flat.is_file();
    let nested_exists = nested.is_file();
    if flat_exists && nested_exists {
        return Err(Diagnostic::type_error(
            format!(
                "ambiguous module layout for `{name}`: both `{}` and `{}` exist",
                flat.display(),
                nested.display()
            ),
            1,
            1,
        ));
    }
    if flat_exists {
        return Ok(Some(flat));
    }
    if nested_exists {
        return Ok(Some(nested));
    }
    Ok(None)
}

fn expected_module_name_from_path(path: &Path, project_root: &Path) -> XResult<String> {
    let file_name = path
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| Diagnostic::io("invalid module file path", 1, 1))?;

    if file_name.ends_with(".x") && path.parent() == Some(project_root) {
        return Ok(file_name.trim_end_matches(".x").to_owned());
    }

    if file_name == "main.x" {
        let parent = path
            .parent()
            .and_then(Path::file_name)
            .and_then(|name| name.to_str())
            .ok_or_else(|| Diagnostic::io("invalid nested module path", 1, 1))?;
        return Ok(parent.to_owned());
    }

    Err(Diagnostic::type_error(
        format!(
            "module file `{}` must be `{{name}}.x` or `{{name}}/main.x` under project root",
            path.display()
        ),
        1,
        1,
    ))
}

fn format_cycle(stack: &[String], closing: &str) -> String {
    let mut parts: Vec<&str> = stack.iter().map(String::as_str).collect();
    parts.push(closing);
    parts.push(stack.first().map(String::as_str).unwrap_or(closing));
    parts.join(" → ")
}
