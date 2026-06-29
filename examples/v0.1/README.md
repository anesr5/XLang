# XLang v0.1 Examples

Sample programs for the **v0.1 language subset**. Each file is self-contained and targets the bootstrap compiler in `compiler/`.

## Running

```bash
# from repository root
cargo run --manifest-path compiler/Cargo.toml -- run examples/v0.1/main.x
cargo run --manifest-path compiler/Cargo.toml -- check examples/v0.1/invalid_missing_semicolon.x
```

## Programs

| File | Expected result |
|------|-----------------|
| `main.x` | Exit code **42** — demo with functions, `const`, `if`, assignment |
| `control_flow.x` | Exit code **42** — comparisons and short-circuit logic |
| `struct_declarations.x` | Exit code **0** — struct syntax (declarations only) |
| `invalid_missing_semicolon.x` | Parse error — missing `;` |
| `invalid_immutable_assignment.x` | Type error — assign to `const` local |
| `invalid_division_by_zero.x` | Type error — literal division by zero |
| `invalid_unreachable.x` | Type error — statement after `return` |

## Syntax reminder

```xlang
i32 name(i32 param) { … }      // function
i32 x = expr;                  // mutable local
const i32 x = expr;            // immutable local
```

There is no `fn`, `let`, or `var` keyword in v0.1.
