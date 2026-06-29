# Limitations

Explicit list of what **XLang v0.1 does not provide**, even if tokens or keywords exist in the lexer.

## Language features

| Not supported | Notes |
|---------------|-------|
| Type inference for locals | Type required on every binding |
| Multiple files / linking | Single translation unit only |
| Import resolution | `import` is parsed, ignored |
| Module path mapping | `module` is parsed, ignored |
| Struct values | Declarations only — no construction, fields, or methods |
| Enums, traits, impls | Keywords reserved |
| `match`, loops (`for`, `while`, `loop`) | Not in grammar |
| `break`, `continue`, `defer` | Not in grammar |
| `if` expressions | `if` is statement-only |
| `else if` sugar | Nest `if` in `else` block |
| Generic types | Not supported |
| Ownership / borrowing | Not supported |
| `unsafe` blocks | Not supported |
| Error types / `Result` | Not supported |
| Macros | Not supported |
| Async / await / concurrency | Not supported |
| GPU (`gpu`, `spawn`, …) | Not supported |
| C FFI / extern functions | Not supported |
| Standard library | Not included |

## Types and literals

| Not supported in expressions | Lexer may tokenize |
|------------------------------|-------------------|
| Float literals | yes (`3.14`) |
| Character literals | yes (`'a'`) |
| Integer types other than `i32` | — |
| Unsigned integers | — |
| `char` type | — |

## Operators lexed but not in expression grammar

`&` `|` `^` `~` `<<` `>>` `->` `=>` `.` `::` `?` `[` `]`

## Comments

| Form | Status |
|------|--------|
| Nested `/* ... */` | not supported |
| Structured documentation comments | `///` is skipped lexically; no doc metadata is retained |

## UTF-8 BOM

Files starting with BOM are rejected as an unknown character.

## Semantic gaps (known)

| Gap | Detail |
|-----|--------|
| Full constant evaluation | Only literal division/remainder by zero is diagnosed |
| `check` vs codegen | `str` programs may pass `check` only |
| Fine-grained diagnostic codes | Family-level codes only |

## Tooling gaps

- No official formatter
- No stable/official LSP release; experimental editor analysis exists outside the MVP compiler contract
- No cross-compilation guarantees beyond passing `-target` to IR and `clang`
- Build output paths are fixed (`build/main.ll`, `build/main.exe`)

## Reserved for future RFCs

Features described in `docs/rfcs/` but **not** in this spec are planned or exploratory, not implemented. This spec takes precedence over RFC drafts for current behavior.

When in doubt, treat the compiler and `docs/spec/` as the source of truth:

```bash
cargo test --manifest-path compiler/Cargo.toml
x check your_program.x
x run your_program.x
```
