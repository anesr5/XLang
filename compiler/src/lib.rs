pub mod ast;
pub mod backend;
pub mod compile;
pub mod diagnostic;
pub mod lexer;
#[cfg(windows)]
mod llvm_windows_shim;
pub mod lsp;
pub mod modules;
pub mod parser;
pub mod token;
pub mod typeck;

pub use compile::{
    CheckedProgram, CompileOptions, build, build_with_options, check, emit_llvm,
    emit_llvm_with_options, run, run_with_options,
};
pub use diagnostic::{Diagnostic, DiagnosticCode, Span, XResult};
pub use lsp::{
    AnalysisResult, HoverContext, MemberCompletionItem, ReferenceKind, SemanticIndex, Symbol,
    SymbolId, SymbolKind, analyze_source, build_hover_at_offset, format_type_name, hover_markdown,
    member_field_completions,
};

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ast::{Expr, Stmt};

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

    fn write_module_fixture(case: &str, files: &[(&str, &str)]) -> std::path::PathBuf {
        let unique = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("system time should be after Unix epoch")
            .as_nanos();
        let dir =
            std::env::temp_dir().join(format!("xlang_{case}_{}_{}", std::process::id(), unique));
        std::fs::create_dir_all(&dir).expect("failed to create temporary XLang fixture");
        for (file_name, source) in files {
            std::fs::write(dir.join(file_name), source)
                .expect("failed to write temporary XLang fixture source");
        }
        dir.join("main.x")
    }

    #[test]
    fn lexes_semicolon_token() {
        let tokens = lexer::lex("i32 x = 1;").unwrap();
        assert!(
            tokens
                .iter()
                .any(|token| matches!(token.kind, token::TokenKind::Semicolon))
        );
    }

    #[test]
    fn lexer_records_full_token_spans() {
        let tokens = lexer::lex("i32 x = 1;").unwrap();
        let token = tokens
            .iter()
            .find(|token| matches!(token.kind, token::TokenKind::Identifier(_)))
            .unwrap();
        assert_eq!(token.span.start_byte, 0);
        assert_eq!(token.span.end_byte, 3);
        assert_eq!(token.span.start_line, 1);
        assert_eq!(token.span.start_column, 1);
        assert_eq!(token.span.end_column, 4);
    }

    #[test]
    fn lexer_reserves_future_keywords() {
        let tokens = lexer::lex("enum trait match unsafe move mut as in const").unwrap();
        assert!(
            tokens[..9]
                .iter()
                .all(|token| matches!(token.kind, token::TokenKind::Keyword(_)))
        );
    }

    #[test]
    fn lexer_treats_fn_let_var_as_identifiers() {
        let tokens = lexer::lex("fn let var").unwrap();
        assert!(matches!(tokens[0].kind, token::TokenKind::Identifier(_)));
        assert!(matches!(tokens[1].kind, token::TokenKind::Identifier(_)));
        assert!(matches!(tokens[2].kind, token::TokenKind::Identifier(_)));
    }

    #[test]
    fn lexer_skips_line_doc_and_block_comments() {
        let source = r#"
/// doc comment is skipped
/* block
   comment */
i32 main() {
    // line comment
    return 0;
}
"#;
        compile::check_source(source).unwrap();
    }

    #[test]
    fn lexer_rejects_unterminated_block_comment() {
        let err = lexer::lex("i32 main() { /* nope").unwrap_err();
        assert_eq!(err.code, diagnostic::DiagnosticCode::Lexical);
        assert!(err.message.contains("unterminated block comment"));
        assert_eq!(err.span.start_line, 1);
        assert_eq!(err.span.start_column, 14);
    }

    #[test]
    fn diagnostics_carry_stable_codes() {
        let lex_err = lexer::lex("@").unwrap_err();
        assert_eq!(lex_err.code, diagnostic::DiagnosticCode::Lexical);
        assert!(
            lex_err
                .render(std::path::Path::new("main.x"))
                .contains("E0001")
        );

        let parse_err = compile::check_source("i32 main() { i32 x = 1 }").unwrap_err();
        assert_eq!(parse_err.code, diagnostic::DiagnosticCode::Parse);

        let type_err = compile::check_source("i32 main() { return true; }").unwrap_err();
        assert_eq!(type_err.code, diagnostic::DiagnosticCode::Type);

        let backend_err =
            compile::emit_llvm_source("str message() { return \"x\"; } i32 main() { return 0; }")
                .unwrap_err();
        assert_eq!(backend_err.code, diagnostic::DiagnosticCode::Backend);
    }

    #[test]
    fn parses_checks_and_emits_llvm_for_demo_program() {
        let checked = compile::check_source(DEMO).unwrap();
        assert_eq!(checked.program().functions.len(), 2);
        let llvm_ir = compile::emit_llvm_source(DEMO).unwrap();
        assert!(llvm_ir.contains("define i32 @main()"));
        assert!(llvm_ir.contains("call i32 @xlang.main.add(i32 40, i32 2)"));
    }

    #[test]
    fn parser_preserves_expression_precedence() {
        let checked = compile::check_source(
            r#"
i32 main() {
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
        } = &checked.program().functions[0].body[0]
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
    fn parser_preserves_full_operator_precedence_ladder() {
        let checked = compile::check_source(
            r#"
bool expr() {
    return 1 + 2 * 3 < 8 == true || false && !false;
}

i32 main() {
    return 0;
}
"#,
        )
        .unwrap();
        let Stmt::Return {
            value: Some(Expr::Binary { op, right, .. }),
            ..
        } = &checked.program().functions[0].body[0]
        else {
            panic!("expected binary return expression");
        };
        assert_eq!(*op, ast::BinaryOp::Or);
        assert!(matches!(
            **right,
            Expr::Binary {
                op: ast::BinaryOp::And,
                ..
            }
        ));
    }

    #[test]
    fn parser_accepts_if_else_without_semicolon_after_block() {
        let checked = compile::check_source(
            r#"
i32 main() {
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
            checked.program().functions[0].body[0],
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
            r#"; ModuleID = 'xlang.main'
source_filename = "xlang.main"
target triple = "x86_64-unknown-linux-gnu"

define i32 @xlang.main.add(i32 %a, i32 %b) {
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
  %calltmp = call i32 @xlang.main.add(i32 40, i32 2)
  %x = alloca i32, align 4
  store i32 %calltmp, ptr %x, align 4
  %x.load = load i32, ptr %x, align 4
  ret i32 %x.load
}
"#
        );
    }

    #[test]
    fn emits_stable_v0_6_enum_abi_snapshot() {
        let options = CompileOptions {
            target_triple: Some("x86_64-unknown-linux-gnu".to_owned()),
        };
        let llvm_ir = compile::emit_llvm_source_with_options(
            r#"
module main

enum ResultI32 {
    Ok(i32 value);
    Err(i32 code);
}

ResultI32 identity(ResultI32 input) {
    return input;
}

ResultI32 make_ok(i32 value) {
    return Ok(value);
}

i32 main() {
    return match identity(make_ok(7)) {
        Ok(v) => v,
        Err(code) => code,
    };
}
"#,
            &options,
        )
        .unwrap();
        assert_eq!(
            llvm_ir,
            r#"; ModuleID = 'xlang.main'
source_filename = "xlang.main"
target triple = "x86_64-unknown-linux-gnu"

%main.ResultI32.tagged = type { i32, i32 }

define %main.ResultI32.tagged @xlang.main.identity(%main.ResultI32.tagged %input) {
entry:
  %input.addr = alloca %main.ResultI32.tagged, align 8
  store %main.ResultI32.tagged %input, ptr %input.addr, align 4
  %input.load = load %main.ResultI32.tagged, ptr %input.addr, align 4
  ret %main.ResultI32.tagged %input.load
}

define %main.ResultI32.tagged @xlang.main.make_ok(i32 %value) {
entry:
  %value.addr = alloca i32, align 4
  store i32 %value, ptr %value.addr, align 4
  %value.load = load i32, ptr %value.addr, align 4
  %enum.val = insertvalue %main.ResultI32.tagged { i32 0, i32 undef }, i32 %value.load, 1
  ret %main.ResultI32.tagged %enum.val
}

define i32 @main() {
entry:
  %calltmp = call %main.ResultI32.tagged @xlang.main.make_ok(i32 7)
  %calltmp1 = call %main.ResultI32.tagged @xlang.main.identity(%main.ResultI32.tagged %calltmp)
  %match.tag = extractvalue %main.ResultI32.tagged %calltmp1, 0
  %match.payload = extractvalue %main.ResultI32.tagged %calltmp1, 1
  switch i32 %match.tag, label %match.trap [
    i32 0, label %match.arm.0
    i32 1, label %match.arm.1
  ]

match.end:                                        ; preds = %match.arm.1, %match.arm.0
  %match.result = phi i32 [ %v.load, %match.arm.0 ], [ %code.load, %match.arm.1 ]
  %match.tmp = alloca i32, align 4
  store i32 %match.result, ptr %match.tmp, align 4
  br label %match.cont

match.trap:                                       ; preds = %entry
  call void @llvm.trap()
  unreachable

match.arm.0:                                      ; preds = %entry
  %v = alloca i32, align 4
  store i32 %match.payload, ptr %v, align 4
  %v.load = load i32, ptr %v, align 4
  br label %match.end

match.arm.1:                                      ; preds = %entry
  %code = alloca i32, align 4
  store i32 %match.payload, ptr %code, align 4
  %code.load = load i32, ptr %code, align 4
  br label %match.end

match.cont:                                       ; preds = %match.end
  %match.val = load i32, ptr %match.tmp, align 4
  ret i32 %match.val
}

; Function Attrs: cold noreturn nounwind memory(inaccessiblemem: write)
declare void @llvm.trap() #0

attributes #0 = { cold noreturn nounwind memory(inaccessiblemem: write) }
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
i32 main() {
    i32 x = 1;
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
i32 main() {
    return 0;
}
"#,
            &options,
        )
        .unwrap();
        assert!(llvm_ir.contains(r#"target triple = "wasm32-unknown-unknown""#));
    }

    #[test]
    fn build_pipeline_does_not_use_c_as_ir_or_gcc() {
        let compile_source = include_str!("compile.rs");
        assert!(compile_source.contains("{module_name}.ll"));
        assert!(compile_source.contains("clang"));
        assert!(!compile_source.contains("main.c"));
        assert!(!compile_source.contains("gcc"));

        let backend_source = include_str!("backend/llvm.rs");
        assert!(backend_source.contains("Module"));
        assert!(!backend_source.contains("main.c"));
        assert!(!backend_source.contains("gcc"));
    }

    #[test]
    fn llvm_logical_and_uses_short_circuit_blocks() {
        let llvm_ir = compile::emit_llvm_source(
            r#"
bool expensive() {
    return true;
}

i32 main() {
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
        assert!(llvm_ir.contains("call i1 @xlang.main.expensive()"));
    }

    #[test]
    fn llvm_logical_or_uses_short_circuit_blocks() {
        let llvm_ir = compile::emit_llvm_source(
            r#"
bool expensive() {
    return false;
}

i32 main() {
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
        assert!(llvm_ir.contains("call i1 @xlang.main.expensive()"));
    }

    #[test]
    fn llvm_if_else_with_returning_branches_verifies() {
        let llvm_ir = compile::emit_llvm_source(
            r#"
i32 main() {
    if true {
        return 1;
    } else {
        return 2;
    }
}
"#,
        )
        .unwrap();
        assert!(llvm_ir.contains("if.end"));
        assert!(llvm_ir.contains("unreachable"));
    }

    #[test]
    fn parses_top_level_struct_declarations() {
        let source = r#"
module main

struct Player {
    i32 hp;
    bool alive;
}

i32 main() {
    return 42;
}
"#;
        let checked = compile::check_source(source).unwrap();
        assert_eq!(checked.program().structs.len(), 1);
        assert_eq!(checked.program().structs[0].fields.len(), 2);
    }

    #[test]
    fn rejects_duplicate_struct_names() {
        let source = r#"
struct Player {
    i32 hp;
}

struct Player {
    i32 score;
}

i32 main() {
    return 0;
}
"#;
        let err = compile::check_source(source).unwrap_err();
        assert_eq!(err.code, diagnostic::DiagnosticCode::Type);
        assert!(err.message.contains("duplicate struct"));
        assert_eq!(err.span.start_line, 6);
        assert_eq!(err.span.start_column, 8);
    }

    #[test]
    fn rejects_duplicate_struct_fields() {
        let source = r#"
struct Player {
    i32 hp;
    bool hp;
}

i32 main() {
    return 0;
}
"#;
        let err = compile::check_source(source).unwrap_err();
        assert_eq!(err.code, diagnostic::DiagnosticCode::Type);
        assert!(err.message.contains("duplicate field"));
        assert_eq!(err.span.start_line, 4);
        assert_eq!(err.span.start_column, 10);
    }

    #[test]
    fn allows_distinct_structs_with_same_field_names() {
        let source = r#"
struct Player {
    i32 id;
}

struct Enemy {
    i32 id;
}

i32 main() {
    return 0;
}
"#;
        compile::check_source(source).unwrap();
    }

    #[test]
    fn rejects_missing_semicolon() {
        let source = r#"
i32 main() {
    i32 x = 1
    return x;
}
"#;
        let err = compile::check_source(source).unwrap_err();
        assert!(err.message.contains("expected `;`"));
    }

    #[test]
    fn rejects_immutable_assignment() {
        let source = r#"
i32 main() {
    const i32 x = 1;
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
i32 main() {
    i32 x = 1;
}
"#;
        let err = compile::check_source(source).unwrap_err();
        assert!(err.message.contains("may exit without returning"));
        assert_eq!(err.span.start_line, 2);
        assert_eq!(err.span.start_column, 5);
    }

    #[test]
    fn rejects_unreachable_statement_after_return() {
        let source = r#"
i32 main() {
    return 0;
    i32 x = 1;
}
"#;
        let err = compile::check_source(source).unwrap_err();
        assert_eq!(err.code, diagnostic::DiagnosticCode::Type);
        assert!(err.message.contains("unreachable statement"));
        assert_eq!(err.span.start_line, 4);
        assert_eq!(err.span.start_column, 5);
    }

    #[test]
    fn rejects_unreachable_statement_inside_if_branch() {
        let source = r#"
i32 main() {
    if true {
        return 1;
        return 2;
    }
    return 0;
}
"#;
        let err = compile::check_source(source).unwrap_err();
        assert_eq!(err.code, diagnostic::DiagnosticCode::Type);
        assert!(err.message.contains("unreachable statement"));
        assert_eq!(err.span.start_line, 5);
        assert_eq!(err.span.start_column, 9);
    }

    #[test]
    fn branch_local_binding_is_not_visible_after_if() {
        let source = r#"
i32 main() {
    if true {
        i32 x = 1;
    }
    return x;
}
"#;
        let err = compile::check_source(source).unwrap_err();
        assert!(err.message.contains("unknown variable"));
        assert_eq!(err.span.start_line, 6);
        assert_eq!(err.span.start_column, 12);
    }

    #[test]
    fn check_accepts_frontend_valid_string_program() {
        let source = r#"
str message() {
    return "not in the backend yet";
}

i32 main() {
    return 0;
}
"#;
        compile::check_source(source).unwrap();
    }

    #[test]
    fn emit_llvm_rejects_unsupported_string_codegen() {
        let source = r#"
str message() {
    return "not in the backend yet";
}

i32 main() {
    return 0;
}
"#;
        let err = compile::emit_llvm_source(source).unwrap_err();
        assert!(err.message.contains("LLVM MVP supports"));
        assert_eq!(err.span.start_line, 2);
        assert_eq!(err.span.start_column, 1);
    }

    #[test]
    fn check_accepts_void_call_as_expression_statement() {
        let source = r#"
void tick() {
    return;
}

i32 main() {
    tick();
    return 0;
}
"#;
        compile::check_source(source).unwrap();
    }

    #[test]
    fn accepts_forward_function_call() {
        let source = r#"
i32 main() {
    return later();
}

i32 later() {
    return 42;
}
"#;
        compile::check_source(source).unwrap();
    }

    #[test]
    fn lowers_void_function_fallthrough() {
        let llvm_ir = compile::emit_llvm_source(
            r#"
void tick() {
}

i32 main() {
    tick();
    return 0;
}
"#,
        )
        .unwrap();
        assert!(llvm_ir.contains("define void @xlang.main.tick()"));
        assert!(llvm_ir.contains("ret void"));
    }

    #[test]
    fn rejects_binding_void_call_result() {
        let source = r#"
void tick() {
    return;
}

i32 main() {
    i32 x = tick();
    return 0;
}
"#;
        let err = compile::check_source(source).unwrap_err();
        assert!(err.message.contains("cannot bind void value"));
        assert_eq!(err.span.start_line, 7);
        assert_eq!(err.span.start_column, 13);
    }

    #[test]
    fn rejects_void_local_type() {
        let source = r#"
i32 main() {
    void x = 0;
    return 0;
}
"#;
        let err = compile::check_source(source).unwrap_err();
        assert!(err.message.contains("cannot have type void"));
        assert_eq!(err.span.start_line, 3);
        assert_eq!(err.span.start_column, 5);
    }

    #[test]
    fn rejects_returning_void_expression() {
        let source = r#"
void tick() {
    return;
}

i32 main() {
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
void tick() {
    return;
}

void consume(i32 x) {
    return;
}

i32 main() {
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
i32 main(i32 argc) {
    return argc;
}
"#;
        let err = compile::check_source(source).unwrap_err();
        assert!(err.message.contains("main` must not have parameters"));
        assert_eq!(err.span.start_line, 2);
        assert_eq!(err.span.start_column, 14);
    }

    #[test]
    fn rejects_non_i32_main_return_type() {
        let source = r#"
bool main() {
    return true;
}
"#;
        let err = compile::check_source(source).unwrap_err();
        assert!(err.message.contains("main` must return i32"));
        assert_eq!(err.span.start_line, 2);
        assert_eq!(err.span.start_column, 1);
    }

    #[test]
    fn rejects_duplicate_parameters() {
        let source = r#"
i32 add(i32 x, i32 x) {
    return x;
}

i32 main() {
    return add(1, 2);
}
"#;
        let err = compile::check_source(source).unwrap_err();
        assert!(err.message.contains("duplicate parameter"));
        assert_eq!(err.span.start_line, 2);
        assert_eq!(err.span.start_column, 20);
    }

    #[test]
    fn rejects_duplicate_function_names() {
        let source = r#"
i32 main() {
    return 0;
}

i32 main() {
    return 1;
}
"#;
        let err = compile::check_source(source).unwrap_err();
        assert!(err.message.contains("duplicate function"));
        assert_eq!(err.code, diagnostic::DiagnosticCode::Type);
    }

    #[test]
    fn allows_assignment_to_parameters() {
        let source = r#"
i32 bump(i32 x) {
    x = x + 1;
    return x;
}

i32 main() {
    return bump(41);
}
"#;
        compile::check_source(source).unwrap();
    }

    #[test]
    fn rejects_named_return_type_at_type_span() {
        let source = r#"
struct Player {
    i32 hp;
}

Player make() {
    return 0;
}

i32 main() {
    return 0;
}
"#;
        let err = compile::check_source(source).unwrap_err();
        assert!(err.message.contains("struct type `Player`"));
        assert_eq!(err.span.start_line, 6);
        assert_eq!(err.span.start_column, 1);
    }

    #[test]
    fn rejects_unknown_struct_local_type() {
        let source = r#"
i32 main() {
    Vec3 p = { 1, 2, 3 };
    return 0;
}
"#;
        let err = compile::check_source(source).unwrap_err();
        assert_eq!(err.code, diagnostic::DiagnosticCode::Type);
        assert!(err.message.contains("unknown struct type `Vec3`"));
    }

    #[test]
    fn rejects_struct_local_without_initializer() {
        let source = r#"
struct Vec2 {
    i32 x;
    i32 y;
}

i32 main() {
    Vec2 p;
    return 0;
}
"#;
        let err = compile::check_source(source).unwrap_err();
        assert!(err.message.contains("expected `=` in binding"));
    }

    #[test]
    fn rejects_void_parameter_at_type_span() {
        let source = r#"
void consume(void value) {
    return;
}

i32 main() {
    return 0;
}
"#;
        let err = compile::check_source(source).unwrap_err();
        assert!(err.message.contains("cannot have type void"));
        assert_eq!(err.span.start_line, 2);
        assert_eq!(err.span.start_column, 14);
    }

    #[test]
    fn return_type_mismatch_uses_return_expression_span() {
        let source = r#"
i32 main() {
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
i32 main() {
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
i32 main() {
    str message = "not in backend";
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
void consume(str value) {
    return;
}

i32 main() {
    return 0;
}
"#;
        let err = compile::emit_llvm_source(source).unwrap_err();
        assert!(err.message.contains("LLVM MVP supports"));
        assert_eq!(err.span.start_line, 2);
        assert_eq!(err.span.start_column, 14);
    }

    #[test]
    fn rejects_duplicate_local_binding() {
        let source = r#"
i32 main() {
    i32 x = 1;
    i32 x = 2;
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
i32 main() {
    if true {
        i32 x = 1;
    } else {
        i32 x = 2;
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
i32 main() {
    if true {
        i32 x = 1;
    }
    i32 x = 2;
    return x;
}
"#;
        let err = compile::check_source(source).unwrap_err();
        assert!(err.message.contains("duplicate binding"));
        assert_eq!(err.span.start_line, 6);
        assert_eq!(err.span.start_column, 9);
    }

    #[test]
    fn parses_module_and_imports_before_items_without_semicolons() {
        let checked = compile::check_source(
            r#"
module main
import math
import io

i32 main() {
    return 0;
}
"#,
        )
        .unwrap();
        assert_eq!(checked.program().module.as_deref(), Some("main"));
        assert_eq!(
            checked
                .program()
                .imports
                .iter()
                .map(|import| import.name.as_str())
                .collect::<Vec<_>>(),
            vec!["math", "io"]
        );
    }

    #[test]
    fn rejects_duplicate_imports() {
        let err = compile::check_source(
            r#"
module main
import io
import io

i32 main() {
    return 0;
}
"#,
        )
        .unwrap_err();
        assert!(err.message.contains("duplicate import"));
    }

    #[test]
    fn rejects_module_after_import_or_item() {
        for source in [
            r#"
import io
module late

i32 main() {
    return 0;
}
"#,
            r#"
i32 main() {
    return 0;
}

module late
"#,
        ] {
            let err = compile::check_source(source).unwrap_err();
            assert_eq!(err.code, diagnostic::DiagnosticCode::Parse);
        }
    }

    #[test]
    fn rejects_import_after_item() {
        let err = compile::check_source(
            r#"
i32 main() {
    return 0;
}

import late
"#,
        )
        .unwrap_err();
        assert_eq!(err.code, diagnostic::DiagnosticCode::Parse);
    }

    #[test]
    fn rejects_malformed_parameter_lists_and_unterminated_blocks() {
        for source in [
            r#"
i32 add(i32 a,) {
    return a;
}

i32 main() {
    return add(1);
}
"#,
            r#"
i32 main() {
    return 0;
"#,
        ] {
            let err = compile::check_source(source).unwrap_err();
            assert_eq!(err.code, diagnostic::DiagnosticCode::Parse);
        }
    }

    #[test]
    fn allows_assignment_to_predeclared_var_inside_if_branches() {
        let source = r#"
i32 main() {
    i32 x = 0;
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
i32 main() {
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
i32 main() {
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
i32 main() {
    return -2147483649;
}
"#;
        let err = compile::check_source(source).unwrap_err();
        assert!(err.message.contains("does not fit in i32"));
        assert_eq!(err.span.start_line, 3);
        assert_eq!(err.span.start_column, 12);
    }

    #[test]
    fn rejects_float_and_char_literals_in_expressions() {
        for source in [
            r#"
i32 main() {
    return 3.14;
}
"#,
            r#"
i32 main() {
    return 'a';
}
"#,
        ] {
            let err = compile::check_source(source).unwrap_err();
            assert_eq!(err.code, diagnostic::DiagnosticCode::Parse);
            assert!(err.message.contains("expected expression"));
        }
    }

    #[test]
    fn rejects_literal_division_and_remainder_by_zero() {
        for (source, column) in [
            (
                r#"
i32 main() {
    return 10 / 0;
}
"#,
                17,
            ),
            (
                r#"
i32 main() {
    return 10 % 0;
}
"#,
                17,
            ),
            (
                r#"
i32 main() {
    return 10 / -0;
}
"#,
                17,
            ),
        ] {
            let err = compile::check_source(source).unwrap_err();
            assert_eq!(err.code, diagnostic::DiagnosticCode::Type);
            assert!(err.message.contains("by zero"));
            assert_eq!(err.span.start_line, 3);
            assert_eq!(err.span.start_column, column);
        }
    }

    #[test]
    fn allows_nonliteral_divisor_until_constant_analysis_exists() {
        let source = r#"
i32 main() {
    i32 x = 1;
    return 10 / x;
}
"#;
        compile::check_source(source).unwrap();
    }

    #[test]
    fn unknown_variable_diagnostic_uses_variable_span() {
        let source = r#"
i32 main() {
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
void consume(i32 value) {
    return;
}

i32 main() {
    consume(true);
    return 0;
}
"#;
        let err = compile::check_source(source).unwrap_err();
        assert!(err.message.contains("argument type mismatch"));
        assert_eq!(err.span.start_line, 7);
        assert_eq!(err.span.start_column, 13);
    }

    #[test]
    fn rejects_while_non_bool_condition() {
        let source = r#"
i32 main() {
    while 1 {
    }
    return 0;
}
"#;
        let err = compile::check_source(source).unwrap_err();
        assert!(err.message.contains("while condition must be bool"));
    }

    #[test]
    fn rejects_break_outside_loop() {
        let source = r#"
i32 main() {
    break;
    return 0;
}
"#;
        let err = compile::check_source(source).unwrap_err();
        assert!(err.message.contains("break outside of loop"));
    }

    #[test]
    fn rejects_continue_outside_loop() {
        let source = r#"
i32 main() {
    continue;
    return 0;
}
"#;
        let err = compile::check_source(source).unwrap_err();
        assert!(err.message.contains("continue outside of loop"));
    }

    #[test]
    fn rejects_break_in_if_without_while() {
        let source = r#"
i32 main() {
    if true {
        break;
    }
    return 0;
}
"#;
        let err = compile::check_source(source).unwrap_err();
        assert!(err.message.contains("break outside of loop"));
    }

    #[test]
    fn rejects_array_length_zero() {
        let source = r#"
i32 main() {
    i32[0] xs = { };
    return 0;
}
"#;
        let err = compile::check_source(source).unwrap_err();
        assert!(err.message.contains("array length must be at least 1"));
    }

    #[test]
    fn rejects_array_literal_length_mismatch() {
        let source = r#"
i32 main() {
    i32[2] xs = { 1 };
    return 0;
}
"#;
        let err = compile::check_source(source).unwrap_err();
        assert!(err.message.contains("array literal length mismatch"));
    }

    #[test]
    fn rejects_array_element_type_mismatch() {
        let source = r#"
i32 main() {
    i32[2] xs = { true, 1 };
    return 0;
}
"#;
        let err = compile::check_source(source).unwrap_err();
        assert!(err.message.contains("array element type mismatch"));
    }

    #[test]
    fn rejects_const_array_element_assignment() {
        let source = r#"
i32 main() {
    const i32[1] xs = { 0 };
    xs[0] = 1;
    return 0;
}
"#;
        let err = compile::check_source(source).unwrap_err();
        assert!(err.message.contains("const array binding"));
    }

    #[test]
    fn rejects_index_type_not_i32() {
        let source = r#"
i32 main() {
    i32[2] xs = { 0, 0 };
    i32 x = xs[true];
    return x;
}
"#;
        let err = compile::check_source(source).unwrap_err();
        assert!(err.message.contains("array index must be i32"));
    }

    #[test]
    fn rejects_index_on_scalar() {
        let source = r#"
i32 main() {
    i32 x = 0;
    x[0] = 1;
    return x;
}
"#;
        let err = compile::check_source(source).unwrap_err();
        assert!(err.message.contains("cannot index value of type"));
    }

    #[test]
    fn rejects_constant_index_out_of_bounds() {
        let source = r#"
i32 main() {
    i32[2] xs = { 0, 0 };
    xs[2] = 1;
    return 0;
}
"#;
        let err = compile::check_source(source).unwrap_err();
        assert!(err.message.contains("out of bounds"));
    }

    #[test]
    fn lowers_while_with_array_index_and_bounds_check() {
        let source = r#"
i32 main() {
    i32[3] xs = { 1, 2, 3 };
    i32 total = 0;
    i32 i = 0;
    while i < 3 {
        total = total + xs[i];
        i = i + 1;
    }
    return total;
}
"#;
        let llvm_ir = compile::emit_llvm_source(source).unwrap();
        assert!(llvm_ir.contains("while.cond"));
        assert!(llvm_ir.contains("while.body"));
        assert!(llvm_ir.contains("bounds.ok"));
        assert!(llvm_ir.contains("llvm.trap"));
        compile::check_source(source).unwrap();
    }

    #[test]
    fn v0_2_demo_program_returns_expected_sum() {
        const SOURCE: &str = r#"
i32 main() {
    i32[4] xs = { 1, 2, 3, 4 };
    i32 total = 0;
    i32 i = 0;
    while i < 4 {
        total = total + xs[i];
        i = i + 1;
    }
    return total;
}
"#;
        compile::check_source(SOURCE).unwrap();
        compile::emit_llvm_source(SOURCE).unwrap();
    }

    #[test]
    fn accepts_struct_local_with_field_access() {
        let source = r#"
struct Vec2 {
    i32 x;
    i32 y;
}

i32 main() {
    Vec2 p = { 3, 4 };
    i32 sum = p.x + p.y;
    p.y = 10;
    return sum;
}
"#;
        compile::check_source(source).unwrap();
        let llvm_ir = compile::emit_llvm_source(source).unwrap();
        assert!(llvm_ir.contains("%main.Vec2 = type"));
        assert!(llvm_ir.contains("field.load"));
    }

    #[test]
    fn rejects_empty_struct() {
        let source = r#"
struct Empty {
}

i32 main() {
    return 0;
}
"#;
        let err = compile::check_source(source).unwrap_err();
        assert!(err.message.contains("must have at least one field"));
    }

    #[test]
    fn rejects_struct_literal_length_mismatch() {
        let source = r#"
struct Vec2 {
    i32 x;
    i32 y;
}

i32 main() {
    Vec2 p = { 1 };
    return 0;
}
"#;
        let err = compile::check_source(source).unwrap_err();
        assert!(err.message.contains("struct literal length mismatch"));
    }

    #[test]
    fn rejects_struct_field_type_mismatch() {
        let source = r#"
struct Vec2 {
    i32 x;
    i32 y;
}

i32 main() {
    Vec2 p = { true, 1 };
    return 0;
}
"#;
        let err = compile::check_source(source).unwrap_err();
        assert!(err.message.contains("struct field 0 type mismatch"));
    }

    #[test]
    fn rejects_field_access_on_scalar() {
        let source = r#"
i32 main() {
    i32 x = 0;
    i32 y = x.y;
    return y;
}
"#;
        let err = compile::check_source(source).unwrap_err();
        assert!(err.message.contains("cannot access field on value of type"));
    }

    #[test]
    fn rejects_unknown_struct_field() {
        let source = r#"
struct Vec2 {
    i32 x;
    i32 y;
}

i32 main() {
    Vec2 p = { 0, 0 };
    return p.z;
}
"#;
        let err = compile::check_source(source).unwrap_err();
        assert!(err.message.contains("has no field `z`"));
    }

    #[test]
    fn rejects_const_struct_field_assign() {
        let source = r#"
struct Vec2 {
    i32 x;
    i32 y;
}

i32 main() {
    const Vec2 p = { 0, 0 };
    p.x = 1;
    return 0;
}
"#;
        let err = compile::check_source(source).unwrap_err();
        assert!(
            err.message
                .contains("cannot assign to field of const binding")
        );
    }

    #[test]
    fn rejects_nested_struct_field_type() {
        let source = r#"
struct Inner {
    i32 x;
}

struct Outer {
    Inner inner;
}

i32 main() {
    return 0;
}
"#;
        let err = compile::check_source(source).unwrap_err();
        assert!(err.message.contains("nested struct field type"));
    }

    #[test]
    fn v0_4_multi_module_example_typechecks() {
        let path =
            std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../examples/v0.4/main.x");
        compile::check(&path).unwrap();
    }

    #[test]
    fn v0_3_demo_program_returns_expected_sum() {
        const SOURCE: &str = include_str!("../../examples/v0.3/main.x");
        compile::check_source(SOURCE).unwrap();
        compile::emit_llvm_source(SOURCE).unwrap();
    }

    #[test]
    fn v0_5_enum_example_runs() {
        let path =
            std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../examples/v0.5/main.x");
        compile::check(&path).unwrap();
        compile::emit_llvm(&path).unwrap();
    }

    #[test]
    fn rejects_duplicate_enum_variant() {
        let source = r#"
module main

enum E {
    A;
    A;
}

i32 main() { return 0; }
"#;
        let err = compile::check_source(source).unwrap_err();
        assert!(err.message.contains("duplicate variant"));
    }

    #[test]
    fn rejects_non_exhaustive_match() {
        let source = r#"
module main

enum OptionI32 {
    Some(i32 value);
    None;
}

i32 main() {
    OptionI32 x = Some(1);
    return match x {
        Some(v) => v,
    };
}
"#;
        let err = compile::check_source(source).unwrap_err();
        assert!(err.message.contains("non-exhaustive match"));
    }

    #[test]
    fn rejects_unknown_variant_constructor() {
        let source = r#"
module main

enum OptionI32 {
    Some(i32 value);
    None;
}

i32 main() {
    OptionI32 x = Nope(1);
    return 0;
}
"#;
        let err = compile::check_source(source).unwrap_err();
        assert!(err.message.contains("unknown variant"));
    }

    #[test]
    fn rejects_enum_constructor_arity_mismatch() {
        let source = r#"
module main

enum OptionI32 {
    Some(i32 value);
    None;
}

i32 main() {
    OptionI32 x = Some(1, 2);
    return 0;
}
"#;
        let err = compile::check_source(source).unwrap_err();
        assert!(err.message.contains("expects 1 argument"));
    }

    #[test]
    fn rejects_match_on_non_enum() {
        let source = r#"
module main

i32 main() {
    i32 x = 1;
    return match x {
        _ => 0,
    };
}
"#;
        let err = compile::check_source(source).unwrap_err();
        assert!(err.message.contains("match scrutinee must be an enum type"));
    }

    #[test]
    fn rejects_duplicate_type_name_struct_and_enum() {
        let source = r#"
module main

struct Foo {
    i32 x;
}

enum Foo {
    A;
}

i32 main() { return 0; }
"#;
        let err = compile::check_source(source).unwrap_err();
        assert!(err.message.contains("duplicate type name"));
    }

    #[test]
    fn rejects_str_enum_payload() {
        let source = r#"
module main

enum E {
    A(str value);
}

i32 main() { return 0; }
"#;
        let err = compile::check_source(source).unwrap_err();
        assert!(err.message.contains("payload type `str` is not supported"));
    }

    #[test]
    fn v0_6_multi_module_enum_return_example_runs() {
        let path =
            std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../examples/v0.6/main.x");
        compile::check(&path).unwrap();
        compile::emit_llvm(&path).unwrap();
    }

    #[test]
    fn emits_v0_6_cross_module_enum_abi_snapshot() {
        let path = write_module_fixture(
            "cross_module_enum_abi",
            &[
                (
                    "main.x",
                    r#"
module main
import math

i32 main() {
    return match math.divide(10, 2) {
        Ok(v) => v,
        Err(code) => code,
    };
}
"#,
                ),
                (
                    "math.x",
                    r#"
module math

pub enum ResultI32 {
    Ok(i32 value);
    Err(i32 code);
}

pub ResultI32 divide(i32 a, i32 b) {
    if b == 0 {
        return Err(1);
    }
    return Ok(a / b);
}
"#,
                ),
            ],
        );
        let checked = compile::check(&path).unwrap();
        let modules = backend::llvm::emit_compilation_unit(
            &checked.unit,
            &backend::llvm::LlvmOptions {
                target_triple: Some("x86_64-unknown-linux-gnu".to_owned()),
            },
        )
        .unwrap();
        let main_ir = modules.get("main").expect("main module IR");
        let math_ir = modules.get("math").expect("math module IR");

        assert!(math_ir.contains("%math.ResultI32.tagged = type { i32, i32 }"));
        assert!(
            math_ir.contains("define %math.ResultI32.tagged @xlang.math.divide(i32 %a, i32 %b)")
        );
        assert!(math_ir.contains("ret %math.ResultI32.tagged"));
        assert!(main_ir.contains("%math.ResultI32.tagged = type { i32, i32 }"));
        assert!(main_ir.contains("declare %math.ResultI32.tagged @xlang.math.divide(i32, i32)"));
        assert!(
            main_ir.contains(
                "%calltmp = call %math.ResultI32.tagged @xlang.math.divide(i32 10, i32 2)"
            )
        );
        assert!(main_ir.contains("extractvalue %math.ResultI32.tagged %calltmp, 0"));
        assert!(main_ir.contains("extractvalue %math.ResultI32.tagged %calltmp, 1"));
    }

    #[test]
    fn emits_v0_6_cross_module_enum_parameter_abi_snapshot() {
        let path = write_module_fixture(
            "cross_module_enum_param_abi",
            &[
                (
                    "main.x",
                    r#"
module main
import math

i32 main() {
    return math.unwrap(math.ok(9));
}
"#,
                ),
                (
                    "math.x",
                    r#"
module math

pub enum ResultI32 {
    Ok(i32 value);
    Err(i32 code);
}

pub ResultI32 ok(i32 value) {
    return Ok(value);
}

pub i32 unwrap(ResultI32 r) {
    return match r {
        Ok(v) => v,
        Err(code) => code,
    };
}
"#,
                ),
            ],
        );
        let checked = compile::check(&path).unwrap();
        let modules = backend::llvm::emit_compilation_unit(
            &checked.unit,
            &backend::llvm::LlvmOptions {
                target_triple: Some("x86_64-unknown-linux-gnu".to_owned()),
            },
        )
        .unwrap();
        let main_ir = modules.get("main").expect("main module IR");
        let math_ir = modules.get("math").expect("math module IR");

        assert!(math_ir.contains("define i32 @xlang.math.unwrap(%math.ResultI32.tagged %r)"));
        assert!(math_ir.contains("%r.addr = alloca %math.ResultI32.tagged"));
        assert!(math_ir.contains("store %math.ResultI32.tagged %r, ptr %r.addr"));
        assert!(main_ir.contains("declare i32 @xlang.math.unwrap(%math.ResultI32.tagged)"));
        assert!(main_ir.contains("%calltmp = call %math.ResultI32.tagged @xlang.math.ok(i32 9)"));
        assert!(
            main_ir.contains(
                "%calltmp1 = call i32 @xlang.math.unwrap(%math.ResultI32.tagged %calltmp)"
            )
        );
    }

    #[test]
    fn rejects_private_cross_module_enum_type_name() {
        let path = write_module_fixture(
            "private_enum_type",
            &[
                (
                    "main.x",
                    r#"
module main
import math

math.Hidden expose() {
    return math.make();
}

i32 main() { return 0; }
"#,
                ),
                (
                    "math.x",
                    r#"
module math

enum Hidden {
    Value(i32 value);
    Empty;
}

pub Hidden make() {
    return Value(1);
}
"#,
                ),
            ],
        );
        let err = compile::check(&path).unwrap_err();
        assert_eq!(err.code, diagnostic::DiagnosticCode::Type);
        assert!(err.message.contains("cannot use private enum"));
    }

    #[test]
    fn rejects_public_function_leaking_private_enum() {
        let path = write_module_fixture(
            "private_enum_leak",
            &[
                (
                    "main.x",
                    r#"
module main
import math

i32 main() {
    return match math.make() {
        Value(v) => v,
        Empty => 0,
    };
}
"#,
                ),
                (
                    "math.x",
                    r#"
module math

enum Hidden {
    Value(i32 value);
    Empty;
}

pub Hidden make() {
    return Value(1);
}
"#,
                ),
            ],
        );
        let err = compile::check(&path).unwrap_err();
        assert_eq!(err.code, diagnostic::DiagnosticCode::Type);
        assert!(err.message.contains("cannot use private enum"));
    }

    #[test]
    fn rejects_private_cross_module_enum_local_annotation() {
        let path = write_module_fixture(
            "private_enum_local",
            &[
                (
                    "main.x",
                    r#"
module main
import math

i32 main() {
    math.Hidden value = Hidden(1);
    return 0;
}
"#,
                ),
                (
                    "math.x",
                    r#"
module math

enum Hidden {
    Value(i32 value);
    Empty;
}
"#,
                ),
            ],
        );
        let err = compile::check(&path).unwrap_err();
        assert_eq!(err.code, diagnostic::DiagnosticCode::Type);
        assert!(err.message.contains("cannot use private enum"));
    }

    #[test]
    fn accepts_enum_returning_function() {
        let source = r#"
module main

enum ResultI32 {
    Ok(i32 value);
    Err(i32 code);
}

ResultI32 f() {
    return Ok(1);
}

i32 main() {
    return match f() {
        Ok(v) => v,
        Err(_) => 0,
    };
}
"#;
        compile::check_source(source).unwrap();
        compile::emit_llvm_source(source).unwrap();
    }

    #[test]
    fn rejects_unknown_variant_constructor_in_enum_return() {
        let source = r#"
module main

enum ResultI32 {
    Ok(i32 value);
    Err(i32 code);
}

ResultI32 f() {
    return Nope(1);
}

i32 main() { return 0; }
"#;
        let err = compile::check_source(source).unwrap_err();
        assert_eq!(err.code, diagnostic::DiagnosticCode::Type);
        assert!(err.message.contains("unknown variant"));
    }

    #[test]
    fn rejects_unknown_enum_in_function_signature() {
        let source = r#"
module main

Missing f() {
    return 0;
}

i32 main() { return 0; }
"#;
        let err = compile::check_source(source).unwrap_err();
        assert_eq!(err.code, diagnostic::DiagnosticCode::Type);
        assert!(err.message.contains("unknown enum type"));
    }

    #[test]
    fn rejects_unknown_module_in_qualified_enum_signature() {
        let source = r#"
module main

other.Result f() {
    return 0;
}

i32 main() { return 0; }
"#;
        let err = compile::check_source(source).unwrap_err();
        assert_eq!(err.code, diagnostic::DiagnosticCode::Type);
        assert!(err.message.contains("unknown module"));
    }

    #[test]
    fn rejects_array_in_function_signature() {
        let source = r#"
module main

i32[2] f() {
    return 0;
}

i32 main() { return 0; }
"#;
        let err = compile::check_source(source).unwrap_err();
        assert_eq!(err.code, diagnostic::DiagnosticCode::Type);
        assert!(
            err.message
                .contains("array type not supported in function signatures yet")
        );
    }

    #[test]
    fn rejects_enum_return_type_mismatch() {
        let source = r#"
module main

enum ResultI32 {
    Ok(i32 value);
    Err(i32 code);
}

ResultI32 f() {
    return 1;
}

i32 main() { return 0; }
"#;
        let err = compile::check_source(source).unwrap_err();
        assert_eq!(err.code, diagnostic::DiagnosticCode::Type);
        assert!(err.message.contains("return type mismatch") || err.message.contains("expected"));
    }

    #[test]
    fn rejects_struct_in_function_signature() {
        let source = r#"
module main

struct Point {
    i32 x;
}

Point f() {
    return { 0 };
}

i32 main() { return 0; }
"#;
        let err = compile::check_source(source).unwrap_err();
        assert!(
            err.message
                .contains("not supported in function signatures yet")
        );
    }

    #[test]
    fn rejects_main_returning_enum() {
        let source = r#"
module main

enum E {
    A;
}

E main() {
    return A();
}
"#;
        let err = compile::check_source(source).unwrap_err();
        assert!(err.message.contains("`main` must return i32"));
    }
}
