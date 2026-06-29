# Types

## Built-in types

| Type | Size / representation (LLVM) | In signatures | In locals | In expressions | In codegen |
|------|------------------------------|:-------------:|:---------:|:--------------:|:----------:|
| `i32` | 32-bit signed integer (`i32`) | yes | yes | yes | yes |
| `bool` | 1-bit integer (`i1`) | yes | yes | yes | yes |
| `void` | no value | return only | no | no* | yes (returns) |
| `str` | not lowered | yes | yes | yes | **no** |

\* Void appears only as the result of typing a void-returning call used as an expression statement; it cannot be bound, returned (except `return;`), or passed as an argument.

## Named types

Struct names and other identifiers used as types (e.g. `Player`) parse successfully in struct field lists but are **rejected in function signatures**:

```xlang
// Parsed in struct body:
struct Player { i32 hp; }

// Rejected by type checker:
Player make() { return 0; }
// error: struct type `Player` is parsed but not supported in function signatures yet
```

## `void` restrictions

| Context | Allowed |
|---------|---------|
| Function return type | yes — use `void name() { … }` |
| Function parameter | **no** |
| Local binding type | **no** |
| Expression value | **no** |
| Return value | **no** — use `return;` without expression |
| Call argument | **no** |
| Expression statement | yes — `void_fn();` |

## Type equality

There are no implicit conversions. These require **exact** type match:

- assignment to a local
- function arguments
- `return` expressions
- annotated local initializer vs annotation
- equality operands (`==`, `!=`) — both sides must have the **same** type

## Integer range

Expression integer literals must fit in `i32`. See [Lexical structure](lexical.md).

## Frontend vs backend gap

Programs using `str` (literals, parameters, return types, or locals) may pass **`x check`** but fail at **`x emit-llvm`** with:

```text
LLVM MVP supports i32, bool, and void code generation only
```

Always run `emit-llvm`, `build`, or `run` to validate codegen for a complete program.

## Unsupported types (not in language today)

Including but not limited to:

- `i8`, `i16`, `i64`, `i128`, unsigned integers, `usize`, `isize`
- `f16`, `f32`, `f64`
- `char`
- arrays, slices, pointers, references
- generic or parameterized types
- struct values as first-class types

Some spellings may tokenize or parse in struct fields; they are not part of the supported type system until specified otherwise.
