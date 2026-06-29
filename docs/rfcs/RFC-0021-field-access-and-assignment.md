# RFC-0021: Field Access and Assignment

## Status

Draft

## Summary

This RFC defines **field read expressions** (`value.field`) and **field assignment statements** (`value.field = expr;`) for XLang v0.3.

Field access applies to **struct locals** held in stack slots. Chained access through nested structs is out of scope until nested struct fields are supported.

---

## 1. Syntax

### Field read (expression)

Extend postfix expressions:

```ebnf
postfix_expr = primary
             | postfix_expr, "[", expr, "]"      /* v0.2 index */
             | postfix_expr, ".", identifier ;    /* v0.3 field */
```

Examples:

```xlang
i32 x = p.x;
bool alive = player.alive;
i32 sum = p.x + p.y;
```

### Field assign (statement)

Extend assignment targets:

```ebnf
assignable   = identifier
             | identifier, { "[", expr, "]" }      /* v0.2 index assign */
             | identifier, ".", identifier ;       /* v0.3 field assign */
```

Examples:

```xlang
p.x = 10;
player.alive = false;
```

Field assignment uses the existing assignment statement form (`name.field = expr;`), analogous to `arr[i] = expr;`.

---

## 2. Typing Rules

### Base value

The base (`p` in `p.x`) must be an identifier bound to a **struct type** in the current function scope.

| Base type | Result |
|-----------|--------|
| Struct `S` | field access allowed |
| `i32`, `bool`, array | `cannot access field on value of type …` |
| Unknown name | existing unknown variable diagnostic |

### Field name

The field must exist on the struct declaration:

```text
error[E0200]: struct `Vec2` has no field `z`
```

Diagnostics should point at the **field identifier** token.

### Result type (read)

`p.x` has the type of field `x` in struct `S`.

### Assignability (write)

In `p.x = expr;`:

1. Binding `p` must be **mutable** (not `const`)
2. `expr` must match the field type exactly

```text
error[E0200]: cannot assign to field of const binding `p`
error[E0200]: field assignment type mismatch: expected I32, got Bool
```

---

## 3. Precedence and Parsing

Field access binds tighter than binary operators, same as indexing:

```text
p.x + 1      →  (p.x) + 1
arr[i].x     →  invalid in v0.3 (arrays of structs not supported)
p.x.y        →  invalid in v0.3 (nested structs not supported)
```

The base of field access must be an **identifier** in v0.3, not an arbitrary expression:

| Form | v0.3 |
|------|------|
| `p.x` | yes |
| `f().x` | no |
| `(p).x` | no (no parens postfix base) |

Future RFCs may add `expr.field` when struct values can flow through calls and temporaries.

---

## 4. L-value Model

Field assignment is a **statement**, not an assignable expression:

```xlang
p.x = 1;        // ok
i32 a = p.x = 1; // reject — no assignment expression
```

For reads, `p.x` is an **r-value** (load from field slot).

For writes, the backend resolves:

1. Load struct local pointer from environment
2. GEP to field index
3. Store expression value

---

## 5. Interaction with Control Flow

Field assignments to mutable struct locals are allowed inside `if` and `while` bodies, same as scalar assignment:

```xlang
if cond {
    p.x = 1;
} else {
    p.x = 2;
}
```

No branch-local struct bindings in v0.3 unless introduced by a future scoped-binding RFC.

---

## 6. AST Representation

Add to `Expr`:

```rust
FieldAccess {
    base: Box<Expr>,      // v0.3: Variable only
    field: String,
    field_span: Span,
    span: Span,
}
```

Add to `Stmt`:

```rust
AssignField {
    name: String,
    name_span: Span,
    field: String,
    field_span: Span,
    value: Expr,
}
```

Alternatively, reuse a unified `AssignLvalue` enum; implementation choice is left to the compiler.

---

## 7. Diagnostics Summary

| Scenario | Message |
|----------|---------|
| Field on non-struct | `cannot access field on value of type I32` |
| Unknown field | `struct \`S\` has no field \`z\`` |
| Const field assign | `cannot assign to field of const binding \`p\`` |
| Type mismatch on assign | `field assignment type mismatch: expected I32, got Bool` |
| Field read on const | allowed (read-only) |

Span targets: field name for unknown field; base name for wrong base type; value expression for type mismatch on assign.

---

## 8. Examples

```xlang
struct Counter {
    i32 value;
}

i32 main() {
    Counter c = { 0 };
    c.value = c.value + 1;
    return c.value;  // 1
}
```

---

## 9. Negative Tests (required)

| Test | Assert |
|------|--------|
| `p.x` where `p: i32` | message + span on field or base |
| `p.z` unknown field | field span |
| `const` struct field assign | const binding error |
| Wrong assign type | value span |

---

## 10. Open Questions

1. Should reading a field of a `const` struct copy the value (yes, by load) — any special rules?
2. When nested structs arrive, is `outer.inner.x` one or two AST nodes? **Proposed: chained `FieldAccess`.**
