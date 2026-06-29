pub mod ast;
pub mod backend;
pub mod compile;
pub mod diagnostic;
pub mod lexer;
#[cfg(windows)]
mod llvm_windows_shim;
pub mod parser;
pub mod token;
pub mod typeck;

pub use compile::{
    CheckedProgram, CompileOptions, build, build_with_options, check, emit_llvm,
    emit_llvm_with_options, run, run_with_options,
};
pub use diagnostic::{Diagnostic, XResult};

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ast::{Expr, Stmt};

    const DEMO: &str = r#"
module main

fn add(a: i32, b: i32) -> i32 {
    return a + b;
}

fn main() -> i32 {
    let x = add(40, 2);
    return x;
}
"#;

    #[test]
    fn lexes_semicolon_token() {
        let tokens = lexer::lex("let x = 1;").unwrap();
        assert!(
            tokens
                .iter()
                .any(|token| matches!(token.kind, token::TokenKind::Semicolon))
        );
    }

    #[test]
    fn lexer_records_full_token_spans() {
        let tokens = lexer::lex("let x = 1;").unwrap();
        let token = tokens
            .iter()
            .find(|token| matches!(token.kind, token::TokenKind::Identifier(_)))
            .unwrap();
        assert_eq!(token.span.start_byte, 4);
        assert_eq!(token.span.end_byte, 5);
        assert_eq!(token.span.start_line, 1);
        assert_eq!(token.span.start_column, 5);
        assert_eq!(token.span.end_column, 6);
    }

    #[test]
    fn lexer_reserves_future_keywords() {
        let tokens = lexer::lex("enum trait match unsafe move mut as in").unwrap();
        assert!(
            tokens[..8]
                .iter()
                .all(|token| matches!(token.kind, token::TokenKind::Keyword(_)))
        );
    }

    #[test]
    fn parses_checks_and_emits_llvm_for_demo_program() {
        let checked = compile::check_source(DEMO).unwrap();
        assert_eq!(checked.program.functions.len(), 2);
        let llvm_ir = compile::emit_llvm_source(DEMO).unwrap();
        assert!(llvm_ir.contains("define i32 @main()"));
        assert!(llvm_ir.contains("call i32 @add(i32 40, i32 2)"));
    }

    #[test]
    fn parser_preserves_expression_precedence() {
        let checked = compile::check_source(
            r#"
fn main() -> i32 {
    return 1 + 2 * 3;
}
"#,
        )
        .unwrap();
        let Stmt::Return {
            value: Some(Expr::Binary {
                left, op, right, ..
            }),
            ..
        } = &checked.program.functions[0].body[0]
        else {
            panic!("expected binary return expression");
        };
        assert_eq!(*op, ast::BinaryOp::Add);
        assert!(matches!(**left, Expr::Integer { value: 1, .. }));
        assert!(matches!(
            **right,
            Expr::Binary {
                op: ast::BinaryOp::Multiply,
                ..
            }
        ));
    }

    #[test]
    fn parser_accepts_if_else_without_semicolon_after_block() {
        let checked = compile::check_source(
            r#"
fn main() -> i32 {
    if true {
        return 1;
    } else {
        return 2;
    }
}
"#,
        )
        .unwrap();
        assert!(matches!(
            checked.program.functions[0].body[0],
            Stmt::If { .. }
        ));
    }

    #[test]
    fn emits_stable_llvm_ir_snapshot_for_demo_program() {
        let options = CompileOptions {
            target_triple: Some("x86_64-unknown-linux-gnu".to_owned()),
        };
        let llvm_ir = compile::emit_llvm_source_with_options(DEMO, &options).unwrap();
        assert_eq!(
            llvm_ir,
            r#"; ModuleID = 'xlang'
source_filename = "xlang"
target triple = "x86_64-unknown-linux-gnu"

define i32 @add(i32 %a, i32 %b) {
entry:
  %a.addr = alloca i32, align 4
  store i32 %a, ptr %a.addr, align 4
  %b.addr = alloca i32, align 4
  store i32 %b, ptr %b.addr, align 4
  %a.load = load i32, ptr %a.addr, align 4
  %b.load = load i32, ptr %b.addr, align 4
  %addtmp = add i32 %a.load, %b.load
  ret i32 %addtmp
}

define i32 @main() {
entry:
  %calltmp = call i32 @add(i32 40, i32 2)
  %x = alloca i32, align 4
  store i32 %calltmp, ptr %x, align 4
  %x.load = load i32, ptr %x, align 4
  ret i32 %x.load
}
"#
        );
    }

    #[test]
    fn llvm_if_assignment_updates_shared_local_slot() {
        let options = CompileOptions {
            target_triple: Some("x86_64-unknown-linux-gnu".to_owned()),
        };
        let llvm_ir = compile::emit_llvm_source_with_options(
            r#"
fn main() -> i32 {
    var x = 1;
    if false {
        x = 2;
    }
    return x;
}
"#,
            &options,
        )
        .unwrap();
        assert!(llvm_ir.contains("%x = alloca i32, align 4"));
        assert!(llvm_ir.contains("store i32 1, ptr %x, align 4"));
        assert!(llvm_ir.contains("store i32 2, ptr %x, align 4"));
        assert!(llvm_ir.contains("%x.load = load i32, ptr %x, align 4"));
        assert!(llvm_ir.contains("ret i32 %x.load"));
    }

    #[test]
    fn emit_llvm_uses_configured_target_triple() {
        let options = CompileOptions {
            target_triple: Some("wasm32-unknown-unknown".to_owned()),
        };
        let llvm_ir = compile::emit_llvm_source_with_options(
            r#"
fn main() -> i32 {
    return 0;
}
"#,
            &options,
        )
        .unwrap();
        assert!(llvm_ir.contains(r#"target triple = "wasm32-unknown-unknown""#));
    }

    #[test]
    fn llvm_logical_and_uses_short_circuit_blocks() {
        let llvm_ir = compile::emit_llvm_source(
            r#"
fn expensive() -> bool {
    return true;
}

fn main() -> i32 {
    if false && expensive() {
        return 1;
    }
    return 0;
}
"#,
        )
        .unwrap();
        assert!(llvm_ir.contains("and.rhs"));
        assert!(llvm_ir.contains("and.end"));
        assert!(llvm_ir.contains("phi i1"));
        assert!(llvm_ir.contains("call i1 @expensive()"));
    }

    #[test]
    fn llvm_logical_or_uses_short_circuit_blocks() {
        let llvm_ir = compile::emit_llvm_source(
            r#"
fn expensive() -> bool {
    return false;
}

fn main() -> i32 {
    if true || expensive() {
        return 1;
    }
    return 0;
}
"#,
        )
        .unwrap();
        assert!(llvm_ir.contains("or.rhs"));
        assert!(llvm_ir.contains("or.end"));
        assert!(llvm_ir.contains("phi i1"));
        assert!(llvm_ir.contains("call i1 @expensive()"));
    }

    #[test]
    fn parses_top_level_struct_declarations() {
        let source = r#"
module main

struct Player {
    hp: i32;
    alive: bool;
}

fn main() -> i32 {
    return 42;
}
"#;
        let checked = compile::check_source(source).unwrap();
        assert_eq!(checked.program.structs.len(), 1);
        assert_eq!(checked.program.structs[0].fields.len(), 2);
    }

    #[test]
    fn rejects_missing_semicolon() {
        let source = r#"
fn main() -> i32 {
    let x = 1
    return x;
}
"#;
        let err = compile::check_source(source).unwrap_err();
        assert!(err.message.contains("expected `;`"));
    }

    #[test]
    fn rejects_immutable_assignment() {
        let source = r#"
fn main() -> i32 {
    let x = 1;
    x = 2;
    return x;
}
"#;
        let err = compile::check_source(source).unwrap_err();
        assert!(err.message.contains("immutable"));
        assert_eq!(err.span.start_line, 4);
        assert_eq!(err.span.start_column, 5);
    }

    #[test]
    fn rejects_missing_return_in_non_void_function() {
        let source = r#"
fn main() -> i32 {
    let x = 1;
}
"#;
        let err = compile::check_source(source).unwrap_err();
        assert!(err.message.contains("may exit without returning"));
        assert_eq!(err.span.start_line, 2);
        assert_eq!(err.span.start_column, 4);
    }

    #[test]
    fn check_accepts_frontend_valid_string_program() {
        let source = r#"
fn message() -> str {
    return "not in the backend yet";
}

fn main() -> i32 {
    return 0;
}
"#;
        compile::check_source(source).unwrap();
    }

    #[test]
    fn emit_llvm_rejects_unsupported_string_codegen() {
        let source = r#"
fn message() -> str {
    return "not in the backend yet";
}

fn main() -> i32 {
    return 0;
}
"#;
        let err = compile::emit_llvm_source(source).unwrap_err();
        assert!(err.message.contains("LLVM MVP supports"));
        assert_eq!(err.span.start_line, 2);
        assert_eq!(err.span.start_column, 17);
    }

    #[test]
    fn check_accepts_void_call_as_expression_statement() {
        let source = r#"
fn tick() {
    return;
}

fn main() -> i32 {
    tick();
    return 0;
}
"#;
        compile::check_source(source).unwrap();
    }

    #[test]
    fn rejects_binding_void_call_result() {
        let source = r#"
fn tick() {
    return;
}

fn main() -> i32 {
    let x = tick();
    return 0;
}
"#;
        let err = compile::check_source(source).unwrap_err();
        assert!(err.message.contains("cannot bind void value"));
        assert_eq!(err.span.start_line, 7);
        assert_eq!(err.span.start_column, 13);
    }

    #[test]
    fn rejects_returning_void_expression() {
        let source = r#"
fn tick() {
    return;
}

fn main() -> i32 {
    return tick();
}
"#;
        let err = compile::check_source(source).unwrap_err();
        assert!(err.message.contains("cannot return a void expression"));
        assert_eq!(err.span.start_line, 7);
        assert_eq!(err.span.start_column, 12);
    }

    #[test]
    fn rejects_void_argument() {
        let source = r#"
fn tick() {
    return;
}

fn consume(x: i32) {
    return;
}

fn main() -> i32 {
    consume(tick());
    return 0;
}
"#;
        let err = compile::check_source(source).unwrap_err();
        assert!(err.message.contains("cannot pass void expression"));
    }

    #[test]
    fn rejects_main_with_parameters() {
        let source = r#"
fn main(argc: i32) -> i32 {
    return argc;
}
"#;
        let err = compile::check_source(source).unwrap_err();
        assert!(err.message.contains("main` must not have parameters"));
        assert_eq!(err.span.start_line, 2);
        assert_eq!(err.span.start_column, 9);
    }

    #[test]
    fn rejects_non_i32_main_return_type() {
        let source = r#"
fn main() -> bool {
    return true;
}
"#;
        let err = compile::check_source(source).unwrap_err();
        assert!(err.message.contains("main` must return i32"));
        assert_eq!(err.span.start_line, 2);
        assert_eq!(err.span.start_column, 14);
    }

    #[test]
    fn rejects_duplicate_parameters() {
        let source = r#"
fn add(x: i32, x: i32) -> i32 {
    return x;
}

fn main() -> i32 {
    return add(1, 2);
}
"#;
        let err = compile::check_source(source).unwrap_err();
        assert!(err.message.contains("duplicate parameter"));
        assert_eq!(err.span.start_line, 2);
        assert_eq!(err.span.start_column, 16);
    }

    #[test]
    fn rejects_named_return_type_at_type_span() {
        let source = r#"
struct Player {
    hp: i32;
}

fn make() -> Player {
    return 0;
}

fn main() -> i32 {
    return 0;
}
"#;
        let err = compile::check_source(source).unwrap_err();
        assert!(err.message.contains("struct type `Player`"));
        assert_eq!(err.span.start_line, 6);
        assert_eq!(err.span.start_column, 14);
    }

    #[test]
    fn rejects_void_parameter_at_type_span() {
        let source = r#"
fn consume(value: void) {
    return;
}

fn main() -> i32 {
    return 0;
}
"#;
        let err = compile::check_source(source).unwrap_err();
        assert!(err.message.contains("cannot have type void"));
        assert_eq!(err.span.start_line, 2);
        assert_eq!(err.span.start_column, 19);
    }

    #[test]
    fn return_type_mismatch_uses_return_expression_span() {
        let source = r#"
fn main() -> i32 {
    return true;
}
"#;
        let err = compile::check_source(source).unwrap_err();
        assert!(err.message.contains("return type mismatch"));
        assert_eq!(err.span.start_line, 3);
        assert_eq!(err.span.start_column, 12);
    }

    #[test]
    fn if_condition_type_error_uses_condition_span() {
        let source = r#"
fn main() -> i32 {
    if 1 {
        return 1;
    }
    return 0;
}
"#;
        let err = compile::check_source(source).unwrap_err();
        assert!(err.message.contains("if condition must be bool"));
        assert_eq!(err.span.start_line, 3);
        assert_eq!(err.span.start_column, 8);
    }

    #[test]
    fn emit_llvm_string_local_error_uses_literal_span() {
        let source = r#"
fn main() -> i32 {
    let message = "not in backend";
    return 0;
}
"#;
        let err = compile::emit_llvm_source(source).unwrap_err();
        assert!(err.message.contains("LLVM MVP supports"));
        assert_eq!(err.span.start_line, 3);
        assert_eq!(err.span.start_column, 19);
    }

    #[test]
    fn emit_llvm_string_parameter_error_uses_type_span() {
        let source = r#"
fn consume(value: str) {
    return;
}

fn main() -> i32 {
    return 0;
}
"#;
        let err = compile::emit_llvm_source(source).unwrap_err();
        assert!(err.message.contains("LLVM MVP supports"));
        assert_eq!(err.span.start_line, 2);
        assert_eq!(err.span.start_column, 19);
    }

    #[test]
    fn rejects_duplicate_local_binding() {
        let source = r#"
fn main() -> i32 {
    let x = 1;
    let x = 2;
    return x;
}
"#;
        let err = compile::check_source(source).unwrap_err();
        assert!(err.message.contains("duplicate binding"));
        assert_eq!(err.span.start_line, 4);
        assert_eq!(err.span.start_column, 9);
    }

    #[test]
    fn rejects_duplicate_binding_across_if_branches() {
        let source = r#"
fn main() -> i32 {
    if true {
        let x = 1;
    } else {
        let x = 2;
    }
    return 0;
}
"#;
        let err = compile::check_source(source).unwrap_err();
        assert!(err.message.contains("duplicate binding"));
        assert_eq!(err.span.start_line, 6);
        assert_eq!(err.span.start_column, 13);
    }

    #[test]
    fn rejects_duplicate_binding_after_if_branch_binding() {
        let source = r#"
fn main() -> i32 {
    if true {
        let x = 1;
    }
    let x = 2;
    return x;
}
"#;
        let err = compile::check_source(source).unwrap_err();
        assert!(err.message.contains("duplicate binding"));
        assert_eq!(err.span.start_line, 6);
        assert_eq!(err.span.start_column, 9);
    }

    #[test]
    fn allows_assignment_to_predeclared_var_inside_if_branches() {
        let source = r#"
fn main() -> i32 {
    var x = 0;
    if true {
        x = 1;
    } else {
        x = 2;
    }
    return x;
}
"#;
        compile::check_source(source).unwrap();
    }

    #[test]
    fn rejects_integer_literal_above_i32_max() {
        let source = r#"
fn main() -> i32 {
    return 2147483648;
}
"#;
        let err = compile::check_source(source).unwrap_err();
        assert!(err.message.contains("does not fit in i32"));
        assert_eq!(err.span.start_line, 3);
        assert_eq!(err.span.start_column, 12);
    }

    #[test]
    fn accepts_i32_min_literal() {
        let source = r#"
fn main() -> i32 {
    return -2147483648;
}
"#;
        compile::check_source(source).unwrap();
        let llvm_ir = compile::emit_llvm_source(source).unwrap();
        assert!(llvm_ir.contains("ret i32 -2147483648"));
    }

    #[test]
    fn rejects_integer_literal_below_i32_min() {
        let source = r#"
fn main() -> i32 {
    return -2147483649;
}
"#;
        let err = compile::check_source(source).unwrap_err();
        assert!(err.message.contains("does not fit in i32"));
        assert_eq!(err.span.start_line, 3);
        assert_eq!(err.span.start_column, 12);
    }

    #[test]
    fn unknown_variable_diagnostic_uses_variable_span() {
        let source = r#"
fn main() -> i32 {
    return missing;
}
"#;
        let err = compile::check_source(source).unwrap_err();
        assert!(err.message.contains("unknown variable"));
        assert_eq!(err.span.start_line, 3);
        assert_eq!(err.span.start_column, 12);
    }

    #[test]
    fn argument_type_mismatch_uses_argument_span() {
        let source = r#"
fn consume(value: i32) {
    return;
}

fn main() -> i32 {
    consume(true);
    return 0;
}
"#;
        let err = compile::check_source(source).unwrap_err();
        assert!(err.message.contains("argument type mismatch"));
        assert_eq!(err.span.start_line, 7);
        assert_eq!(err.span.start_column, 13);
    }
}
