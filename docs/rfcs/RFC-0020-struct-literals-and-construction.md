# RFC-0020: Struct Literals and Construction

## Status

Draft

## Summary

This RFC defines **struct literals** and **struct local construction** for XLang v0.3.

Struct locals are created exclusively through **positional struct literals** that initialize every field in declaration order.

---

## 1. Syntax

### Struct local binding

Extend binding statements (same form as scalars and arrays):

```ebnf
binding_stmt   = [ "const" ], type_name, identifier, [ array_suffix ], [ "=", initializer ], ";" ;
struct_literal = "{", [ expr, { ",", expr } ], "}" ;
initializer    = expr | array_literal | struct_literal ;
```

Examples:

```xlang
struct Vec2 {
    i32 x;
    i32 y;
}

Vec2 origin = { 0, 0 };
const Vec2 unit = { 1, 1 };
```

### Disambiguation with array literals

Both use `{ … }` braces. The **annotated type** disambiguates:

| Annotation | Literal | Meaning |
|------------|---------|---------|
| `i32[3]` | `{ 1, 2, 3 }` | array literal |
| `Vec2` | `{ 1, 2 }` | struct literal |

Without a struct/array annotation, `{ … }` alone is invalid in expression position in v0.3.

---

## 2. Typing Rules

1. The binding annotation must name a **declared struct type** with a layout entry.
2. The literal must contain **exactly `N` expressions** where `N` is the field count.
3. Expression `i` must match field `i`'s type exactly (no implicit conversions).
4. Struct literals infer type **only** from the binding annotation (no standalone `{ 1, 2 }` expression without context in v0.3).

### Diagnostics

```text
error[E0200]: struct literal length mismatch: expected 2 fields, got 3
error[E0200]: struct field 1 type mismatch: expected I32, got Bool
error[E0200]: unknown struct type `Vec3`
error[E0200]: struct local `Vec2` requires a struct literal initializer
```

---

## 3. Uninitialized Structs

**Decision for v0.3:** uninitialized struct locals are **not** supported.

Every struct local must have a struct literal initializer.

Rationale: same policy as v0.2 arrays — avoid undefined stack contents before definite-assignment rules exist.

Future RFC may add:

```xlang
Vec2 p;
p.x = 1;
p.y = 2;
```

once definite assignment is specified.

---

## 4. Mutability

| Form | Field assign | Rebind variable |
|------|:------------:|:-----------------:|
| `Vec2 p = { … };` | yes (`p.x = …`) | no |
| `const Vec2 p = { … };` | no | no |

`const` applies to the **binding**, not deep immutability of aggregate contents beyond field assign rules in RFC-0021.

---

## 5. AST Representation

Add to `Expr`:

```rust
StructLiteral {
    fields: Vec<Expr>,   // positional, length == struct.field_count
    span: Span,
}
```

The type checker attaches the resolved `StructLayout` via inference from context (binding annotation).

Optional future: `NamedStructLiteral { name, fields: Vec<(String, Expr)> }`.

---

## 6. Lowering Overview

Struct literal lowering (detail in RFC-0022):

1. `alloca` an instance of `%StructName`
2. For each field index `i`, evaluate literal expression `i`
3. `getelementptr` to field `i`, `store` value

This is equivalent to:

```text
tmp = alloca %StructName
store lit0, gep tmp, 0, 0
store lit1, gep tmp, 0, 1
```

The local binding name refers to the `alloca` pointer (same model as scalars and arrays).

---

## 7. Parser Notes

- Reuse existing `{ … }` comma-separated list parser from array literals
- After parsing elements, the **caller** (binding parser) decides array vs struct based on type annotation
- Trailing comma after last element: **reject** (consistent with array literals in v0.2)

---

## 8. Examples

### Valid

```xlang
struct RGB {
    i32 r;
    i32 g;
    i32 b;
}

i32 main() {
    RGB c = { 255, 128, 0 };
    return c.r + c.g + c.b;
}
```

### Invalid

```xlang
RGB c = { 255, 128 };           // length mismatch
RGB c = { true, 128, 0 };       // field 0 type mismatch
RGB c;                          // missing initializer
i32 x = { 1, 2, 3 };            // not a struct type
```

---

## 9. Negative Tests (required)

| Test | Message fragment |
|------|------------------|
| Too few/many literal fields | `struct literal length mismatch` |
| Wrong field type | `struct field N type mismatch` |
| Missing initializer | `requires a struct literal initializer` |
| Literal with non-struct annotation | `expected struct literal` or type mismatch |

---

## 10. Open Questions

1. Named field literals (`.x = 1`) — defer to v0.4?
2. Shallow copy via assignment `a = b` for struct locals — defer (not in v0.3 scope per RFC-0018)?
