# RFC-0019: Struct Layout and Declarations

## Status

Draft

## Summary

This RFC defines **struct declarations**, **layout resolution**, and **field type rules** for XLang v0.3.

Struct declarations are already parsed in v0.1; v0.3 makes them **usable as value types** in local bindings with a deterministic, LLVM-compatible memory layout.

---

## 1. Design Principles

- **Declaration order is layout order** — fields appear in source order; that order is the memory order
- **Natural alignment** — each field is aligned to its scalar size (`i32` → 4 bytes, `bool` → 1 byte with trailing struct padding to alignment)
- **No implicit reordering** — the compiler does not reorder fields for packing in v0.3
- **Single file scope** — all structs in a translation unit are visible; no cross-file struct lookup until modules are implemented
- **Unique names** — duplicate struct or field names remain hard errors (existing v0.1 behavior)

---

## 2. Declaration Syntax

Unchanged from v0.1:

```ebnf
struct_decl = "struct", identifier, "{", { field_decl }, "}" ;
field_decl  = type_name, identifier, ";" ;
```

Example:

```xlang
struct Player {
    i32 hp;
    bool alive;
}
```

### Validation rules (existing + extended)

| Rule | Diagnostic |
|------|------------|
| Duplicate struct name in file | `duplicate struct \`Player\`` |
| Duplicate field name in struct | `duplicate field \`hp\` in struct \`Player\`` |
| Empty struct body | `struct \`Empty\` must have at least one field` |
| Unknown field type name | `unknown type \`Foo\` in struct \`Player\`` |
| Unsupported field type | `field type \`str\` is not supported in struct fields yet` |

---

## 3. Supported Field Types (v0.3)

| Field type | Frontend | Codegen (v0.3) |
|------------|:--------:|:--------------:|
| `i32` | yes | yes |
| `bool` | yes | yes |
| Named struct (`Player`) | yes | **no** — nested structs deferred |
| `str` | parse only | **no** |
| `T[N]` array | parse only | **no** |
| `void` | rejected | **no** |

Field types use the same `type_name` production as functions and locals.

---

## 4. Layout Model

### Field index

Each field receives a **field index** `0 .. N-1` in source declaration order.

### LLVM struct type

For struct `S` with fields `f0: T0, f1: T1, …`:

```llvm
%S = type { T0_llvm, T1_llvm, … }
```

Where:

- `i32` → `i32`
- `bool` → `i1`

LLVM applies its own struct layout rules (alignment and tail padding). The XLang compiler **must** use LLVM's struct type for the corresponding field list and must not hand-compute offsets in v0.3 except for documentation/tests.

### Size and alignment (informative)

For scalar-only structs, expect C-like layout on the target triple:

```xlang
struct Example {
    i32 a;   // offset 0, size 4
    bool b;  // offset 4, size 1
}           // total size 8 (padding after bool)
```

Exact sizes are target-dependent; tests should assert **field index GEP** behavior, not hard-coded byte offsets unless pinned to a triple.

---

## 5. Struct Type Names in the Type System

After layout resolution, a declared struct name `Player` becomes a **named value type** usable in:

- local binding annotations: `Player p = { … };`
- field access result types
- struct literal typing

Still **rejected in v0.3**:

- function parameters and return types
- array element types
- nested struct field types

```text
error[E0200]: struct type `Player` is not supported in function signatures yet
error[E0200]: struct type `Player` is not supported as array element type yet
error[E0200]: nested struct field type `Inner` is not supported yet
```

---

## 6. Layout Table (Compiler Internal)

The type checker builds a **layout table** before checking function bodies:

```text
StructLayout {
    name: "Player",
    fields: [
        { name: "hp",    ty: I32,  index: 0 },
        { name: "alive", ty: Bool, index: 1 },
    ],
    llvm_name: "Player",   // matches XLang struct name in IR
}
```

Lookups:

- `resolve_type("Player")` → `TypeName::Named("Player")` with layout entry
- `field_index("Player", "hp")` → `0`
- `field_type("Player", "hp")` → `I32`

---

## 7. Relationship to Existing Parse-Only Structs

v0.1 behavior:

- Struct declarations parse into `Program.structs`
- Named types in function signatures are rejected

v0.3 extends without breaking syntax:

- Same `struct { … }` declaration form
- LSP hover already shows fields; v0.3 updates status from "parsed only" to "supported in locals"

---

## 8. Negative Tests (required)

| Scenario | Expected |
|----------|----------|
| Empty struct | type error |
| Field type `str` in struct used in codegen path | type or backend error |
| Field type nested struct | type error |
| Unknown type in field list | type error |
| Use undeclared struct name as local type | type error |

See [RFC-0013 § v0.3](RFC-0013-diagnostics-and-error-codes.md).

---

## 9. Open Questions

1. Minimum field count: allow single-field structs? **Proposed: yes.**
2. Maximum field count for MVP? **Proposed: 64** (implementation limit, not language rule).
3. Should forward references (`struct A { B b; } struct B { i32 x; }`) be allowed when nested structs land?
