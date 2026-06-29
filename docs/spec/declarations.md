# Declarations

## Module declaration

Optional. Appears at most once, before imports and items.

```xlang
module main
```

- Name must be a single identifier (no dots).
- Not semicolon-terminated.
- **No semantic effect** today — not checked against file path or other modules.

## Import declarations

Zero or more, after optional `module`, before items.

```xlang
import io
import math
```

- Each import is a single identifier.
- Not semicolon-terminated.
- **No semantic effect** today — imports are not resolved or linked.

## Struct declarations

```xlang
struct Pair {
    i32 left;
    i32 right;
}
```

| Property | Behavior |
|----------|----------|
| Syntax | `struct Name { type field; … }` |
| Field order | Preserved in AST |
| Visibility | All structs in file are parsed; no `pub` yet |
| Use in code | **Not supported** — no construction, field access, or struct-typed signatures |

Struct declarations do not produce LLVM types or symbols.

## Function declarations

```xlang
return_type name(type param, type param, …) {
    { statements }
}
```

### Rules

- **Return type first**, then name, then parameter list.
- Parameters use **type-first** syntax: `i32 a`, not `a: i32`.
- Parameter names must be unique within the function.
- Function names must be unique within the program.
- Parameters are **mutable** locals in the function body (assignable).
- All functions must be fully defined in the same file (no prototypes-only or external linkage).

### Return-type requirements

| Return type | Body requirement |
|-------------|------------------|
| Non-`void` (e.g. `i32`) | Every control-flow path must return a value of that type (see [Semantics](semantics.md)) |
| `void` | May use `return;` or fall off the end (implicit `ret void` in codegen) |

### Forward and mutual recursion

All functions are declared to LLVM before bodies are emitted. Mutual recursion is supported:

```xlang
i32 even(i32 n) {
    if n == 0 { return 1; }
    return odd(n - 1);
}

i32 odd(i32 n) {
    if n == 0 { return 0; }
    return even(n - 1);
}
```

## Local declarations

Inside function bodies only.

```xlang
i32 count = 0;           // mutable
const i32 max = 100;     // immutable
bool done = false;
```

| Form | Mutable | Reassignable |
|------|:-------:|:------------:|
| `type name = expr;` | yes | yes |
| `const type name = expr;` | no | no |

Rules:

- Type is **required** on every local declaration.
- Initializer type must match the declared type (exact match).
- Cannot bind a `void` expression result.
- Name must not duplicate any parameter or prior binding in the **function-wide** binding namespace (see [Semantics](semantics.md)).

Locals declared inside an `if` branch **reserve their name** for the whole function but are **not visible** after the `if`.

```xlang
i32 main() {
    if true {
        i32 x = 1;
    }
    return x;   // error: unknown variable `x`
}
```

Assignment to a mutable local declared outside an `if` is allowed inside branches:

```xlang
i32 main() {
    i32 x = 0;
    if true {
        x = 1;
    } else {
        x = 2;
    }
    return x;
}
```

## The `main` function

Required signature:

```xlang
i32 main() {
    return 0;
}
```

| Rule | Detail |
|------|--------|
| Must exist | Exactly one function named `main` |
| Return type | Must be `i32` |
| Parameters | Must be empty |
| Exit code | Return value used as process exit code when run via `x run` |
