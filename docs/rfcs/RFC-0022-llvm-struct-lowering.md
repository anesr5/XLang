# RFC-0022: LLVM Struct Lowering

## Status

Draft

## Summary

This RFC defines how XLang v0.3 **struct values** lower to LLVM IR through Inkwell.

Struct locals live in **`alloca` slots** pointing to named LLVM struct types. Field access uses **`getelementptr`** + **`load`** / **`store`**, consistent with scalar and array lowering in v0.1/v0.2.

---

## 1. LLVM Type Mapping

For each XLang struct declaration `S` with fields `(T0, T1, …, Tn-1)`:

```llvm
%S = type { T0', T1', …, Tn-1' }
```

Scalar mapping (unchanged):

| XLang | LLVM |
|-------|------|
| `i32` | `i32` |
| `bool` | `i1` |

Struct types are **opaque-named** in IR using the XLang struct identifier (`%Player`, `%Vec2`).

Duplicate struct names are rejected in the frontend before IR emission.

---

## 2. Local Storage Model

### Struct local binding

```xlang
Vec2 p = { 3, 4 };
```

Lowers to:

```llvm
%p = alloca %Vec2, align 8
; initialize fields
%gep0 = getelementptr inbounds %Vec2, ptr %p, i32 0, i32 0
store i32 3, ptr %gep0
%gep1 = getelementptr inbounds %Vec2, ptr %p, i32 0, i32 1
store i32 4, ptr %gep1
```

The `Local` environment maps `p` to:

```text
Local {
    pointer: ptr to %Vec2,
    ty: Named("Vec2"),
    llvm_struct: %Vec2,
}
```

Same **pointer-to-value** representation as scalars (`alloca i32`) and arrays (`alloca [N x T]`).

---

## 3. Field Read

```xlang
i32 x = p.x;
```

For field index `i` (resolved at compile time from layout table):

```llvm
%p.val = load ptr, ptr %p.slot        ; if needed — or use stored pointer directly
%gep = getelementptr inbounds %Vec2, ptr %p.val, i32 0, i32 i
%loaded = load i32, ptr %gep
```

Field index must be a **compile-time constant** in GEP (always true for named field access).

Result type follows field scalar mapping (`i32` or `i1`).

---

## 4. Field Assignment

```xlang
p.x = expr;
```

```llvm
%v = … emit expr …
%gep = getelementptr inbounds %Vec2, ptr %p.ptr, i32 0, i32 i
store i32 %v, ptr %gep
```

`const` bindings are rejected in the type checker; backend assumes mutability.

---

## 5. Struct Literal Initialization

Struct literals do **not** produce SSA struct values. They always initialize an `alloca`:

```text
1. alloca %S
2. for each field index i:
     eval literal[i]
     gep(struct, ptr, 0, i)
     store value
3. bind local name to alloca pointer
```

This matches array literal initialization (element-wise store) from v0.2.

---

## 6. Module Construction Order

1. Emit struct type definitions (`context.struct_type` / named struct types) for all declared structs **before** function bodies
2. Emit functions
3. Within each function, emit allocas in the entry block (consistent with existing scalar/array policy)

Inkwell API sketch:

```rust
let struct_ty = context.struct_type(&[i32_ty, i1_ty], false);
let named = struct_ty.set_name("Player");
```

Exact Inkwell calls are implementation details; IR must verify.

---

## 7. Unsupported Backend Cases

Reject before or during lowering with `E0300`:

| Case | Message |
|------|---------|
| Struct field type `str` | `LLVM backend does not support struct field type str` |
| Nested struct field | `LLVM backend does not support nested struct fields yet` |
| Struct in function signature | `LLVM backend does not support struct parameter type` |
| Array of structs | `LLVM backend does not support struct array element type` |

Frontend should catch most cases; backend guards remain for defense in depth.

---

## 8. Verification and Snapshots

All struct programs must pass `Module::verify()`.

Add IR snapshot tests with:

- struct type definition present
- field GEP indices matching layout order
- stores from struct literal initialization

Pin target triple same as v0.1/v0.2 snapshot tests.

Example expected fragment:

```llvm
%Vec2 = type { i32, i32 }

define i32 @main() {
entry:
  %p = alloca %Vec2, align 8
  %0 = getelementptr inbounds %Vec2, ptr %p, i32 0, i32 0
  store i32 3, ptr %0
  …
}
```

---

## 9. Relationship to RFC-0006

This RFC is the authoritative v0.3 extension to [RFC-0006](RFC-0006-llvm-ir-lowering-rules.md).

RFC-0006 § v0.3 (to be added at implementation time) will cross-reference this document and summarize:

- Named struct types in module
- Struct alloca locals
- GEP field access pattern
- No by-value struct ABI yet

---

## 10. Performance Notes (non-normative)

v0.3 intentionally uses stack `alloca` + load/store per field access. Future optimization:

- `mem2reg` promotion for scalars extracted from structs
- SSA struct values if/when by-value ABI exists

Correctness and verifier passage take priority over optimal IR in v0.3.

---

## 11. Negative / Guard Tests

| Test | Layer |
|------|-------|
| Struct with `str` field reaches codegen | backend error |
| Struct param in IR path | frontend + backend |
| Invalid GEP index (should be impossible if layout table correct) | ICE / debug assert |

---

## 12. Open Questions

1. Explicit `align` on struct `alloca` — use LLVM default or query datalayout?
2. Should struct types be packed (`packed struct`) for C interop later? **Proposed: no in v0.3.**
