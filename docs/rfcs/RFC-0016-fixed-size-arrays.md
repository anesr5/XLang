# RFC-0016: Fixed-Size Stack Arrays

## Status

Draft

## Summary

This RFC defines **fixed-size arrays** allocated on the stack for XLang v0.2.

Arrays have a compile-time-known length, live for the enclosing function scope, and lower to LLVM fixed-vector/array types (`[N x T]`) in `alloca` slots.

Dynamic arrays, heap allocation, slices, and array function parameters are **out of scope** for v0.2 unless explicitly noted otherwise in RFC-0014.

---

## 1. Design Principles

- **Fixed size at compile time** — length is a positive integer literal in the type
- **Stack only** — no heap `new`, no runtime resize
- **Explicit element type** — same as scalars supported by codegen (`i32`, `bool` in v0.2 backend)
- **C-style spelling** — consistent with v0.1 local and parameter syntax
- **No decay to pointer** — arrays are first-class typed values in the frontend; lowering uses known `N`

---

## 2. Type Syntax

```ebnf
array_type    = element_type, "[", integer_literal, "]" ;
element_type  = "i32" | "bool" ;   /* v0.2 codegen subset */
```

Examples:

```xlang
i32[4]
bool[8]
```

Named struct element types inside arrays are **rejected in v0.2** until struct layout exists.

`str` arrays are rejected at codegen (same policy as scalar `str`).

`void` element types are invalid.

---

## 3. Declarations and Literals

### Array locals

Extend binding statements:

```ebnf
binding_stmt  = [ "const" ], type_name, identifier, [ array_suffix ], [ "=", initializer ], ";" ;
array_suffix  = "[", integer_literal, "]" ;
initializer   = expr | array_literal ;
array_literal = "{", [ expr, { ",", expr } ], "}" ;
```

Examples:

```xlang
i32[4] data = { 10, 20, 30, 40 };
const i32[3] primes = { 2, 3, 5 };
bool[2] flags = { true, false };
```

### Length rules

1. The **type length** `N` must be a **positive integer literal** (`N >= 1`).
2. An **array literal** must contain **exactly `N` elements**.
3. Each element expression must match `element_type` exactly (no implicit conversions).

```text
error[E0200]: array length must be at least 1
error[E0200]: array literal length mismatch: expected 4 elements, got 3
error[E0200]: array element type mismatch: expected I32, got Bool
```

### Uninitialized arrays

**Decision for v0.2:** uninitialized array locals are **not** supported.

Every array local must have an initializer (array literal).

Rationale: avoids undefined stack contents before definite-assignment rules exist.

Future RFC may add `i32[4] buf;` with definite assignment.

---

## 4. Mutability

| Form | Element assign | Rebind variable |
|------|:--------------:|:-----------------:|
| `i32[N] name = { … };` | yes (`name[i] = …`) | no (not applicable — no rebind syntax) |
| `const i32[N] name = { … };` | no | no |

`const` applies to the binding: element mutation through index assignment is rejected for `const` arrays.

```xlang
const i32[2] xs = { 1, 2 };
xs[0] = 3;   // error: cannot assign through const array
```

---

## 5. Typing

Introduce array types in the type checker alongside scalars.

| Context | Rule |
|---------|------|
| Local annotation | `i32[N]` as `TypeName::Array { elem, len }` |
| Array literal | Inferred as `i32[N]` / `bool[N]` from length and element types |
| Assignment to scalar local | Arrays are not assignable as wholes (no array copying stmt in v0.2) |
| Function parameter | **Postponed in v0.2** — locals only |
| Function return type | **Postponed in v0.2** |
| Equality `==` / `!=` | **Postponed** — no array comparison in v0.2 |

Whole-array assignment (`a = b;`) is not in v0.2.

---

## 6. Size Limits

Implementations may cap maximum `N` to prevent excessive stack usage.

**Recommended MVP cap:** `65535` elements per array, diagnosable at compile time:

```text
error[E0200]: array length 100000 exceeds maximum stack array size 65535
```

Exact cap is implementation-defined but must be documented in release notes.

---

## 7. Memory Model (semantic)

- Arrays live in the function activation record for the enclosing function.
- Storage is uninitialized **only** if implementation later allows omitted initializers; with v0.2 literal requirement, all elements are initialized from the literal lowering.
- No escape — arrays cannot be returned, passed by value, or stored in globals in v0.2.
- Recursion with large arrays is allowed but subject to stack limits of the native platform.

---

## 8. LLVM Lowering (summary)

Full detail: [RFC-0006 § v0.2](RFC-0006-llvm-ir-lowering-rules.md).

| XLang | LLVM |
|-------|------|
| `i32[N]` local | `alloca [N x i32]`, `store` from literal aggregate or element stores |
| `bool[N]` local | `alloca [N x i1]` |

Literal initialization lowers by:

1. Allocating `[N x T]`
2. Storing each element with `getelementptr` + `store`, **or**
3. Building an LLVM constant aggregate and a single `store` when all elements are constants

Element reads/writes use index expressions (RFC-0017).

---

## 9. Grammar Examples (valid v0.2)

```xlang
i32 main() {
    i32[3] xs = { 1, 2, 3 };
    i32 i = 0;
    while i < 3 {
        xs[i] = xs[i] + 1;
        i = i + 1;
    }
    return xs[2];
}
```

Returns `4`.

---

## 10. Diagnostics (summary)

See [RFC-0013 § v0.2](RFC-0013-diagnostics-and-error-codes.md).

| Scenario | Code |
|----------|------|
| Zero or negative length | `E0200` |
| Literal length mismatch | `E0200` |
| Element type mismatch | `E0200` |
| Assign through `const` array | `E0200` |
| Array in unsupported signature | `E0200` |
| `str[N]` or struct array at codegen | `E0300` |

---

## 11. Negative Tests (required)

| Test | Expected |
|------|----------|
| `i32[0] a = { };` | Length error |
| `i32[2] a = { 1 };` | Literal mismatch |
| `i32[2] a = { true, false };` | Element type mismatch |
| `const i32[1] a = { 0 }; a[0] = 1;` | Const assign error |
| `i32[2] f(i32[2] x)` | Unsupported parameter (if params postponed) |

---

## 12. Explicit Non-Goals

- Dynamic length (`i32[]`, vectors, lists)
- Heap-allocated arrays
- Array function parameters and returns
- Multi-dimensional arrays (`i32[3][4]`) — defer to v0.3+
- Array slicing (`arr[1..3]`)
- `sizeof` / `alignof` operators
- Array-to-pointer decay
- Struct arrays and strings in backend

---

## 13. Open Questions

1. When should `i32[N]` function parameters be added (by-value copy vs pointer)?
2. Should empty literal `{ }` ever mean zero-length (never in v0.2)?
3. What is the right maximum `N` for embedded targets?
