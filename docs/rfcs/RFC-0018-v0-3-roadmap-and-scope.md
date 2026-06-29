# RFC-0018: v0.3 Roadmap and Scope

## Status

Draft

## Summary

This RFC defines the scope, goals, and delivery plan for **XLang v0.3** — the first **struct value** milestone after v0.2 arrays and loops.

v0.3 adds **struct layout**, **struct literals**, **field access**, **field assignment**, and **LLVM struct lowering** for stack-allocated struct locals with scalar fields (`i32`, `bool`).

v0.3 deliberately excludes nested struct values in arrays, struct function parameters/returns, methods, generics, heap allocation, and `str` in struct fields at codegen time.

---

## 1. Motivation

v0.1 and v0.2 established:

- C-style functions, locals, and control flow
- Fixed-size stack arrays with bounds checks
- Direct LLVM lowering through Inkwell with `Module::verify()` gates

Struct **declarations** are already parsed and validated (duplicate names/fields), but struct names cannot appear in function signatures, locals, expressions, or generated code.

Real programs need named aggregate values with field-oriented access. v0.3 closes that gap while keeping the compiler small and auditable.

---

## 2. v0.3 Goals

The v0.3 compiler should:

1. Resolve struct declarations into a canonical **layout table** (field order, offsets, LLVM type)
2. Allow **struct-typed locals** with **struct literals**
3. Parse and type-check **field read** (`value.field`) and **field assign** (`value.field = expr;`)
4. Lower struct locals to LLVM **named struct types** in `alloca` slots with `getelementptr` field access
5. Pass `Module::verify()` for all supported programs
6. Expand the test suite with **negative diagnostic tests** for every new error path
7. Update language reference and release documentation for v0.3

---

## 3. In Scope (v0.3)

| Feature | RFC |
|---------|-----|
| Struct layout and declaration semantics | [RFC-0019](RFC-0019-struct-layout-and-declarations.md) |
| Struct literals and construction | [RFC-0020](RFC-0020-struct-literals-and-construction.md) |
| Field access (read) | [RFC-0021](RFC-0021-field-access-and-assignment.md) |
| Field assignment (write) | [RFC-0021](RFC-0021-field-access-and-assignment.md) |
| LLVM struct lowering | [RFC-0022](RFC-0022-llvm-struct-lowering.md) |
| New diagnostics and negative tests | [RFC-0013](RFC-0013-diagnostics-and-error-codes.md) § v0.3 |

### Syntax additions (summary)

```xlang
struct Vec2 {
    i32 x;
    i32 y;
}

i32 main() {
    Vec2 p = { 3, 4 };
    i32 sum = p.x + p.y;
    p.y = 10;
    return sum;  // 7
}
```

---

## 4. Explicit Non-Goals (v0.3)

| Excluded | Reason |
|----------|--------|
| Struct parameters / return types | ABI and copy semantics not specified yet |
| Struct fields of type `str` at codegen | String ABI still unspecified |
| Struct fields of type `Array` | Nested aggregate layout deferred |
| Nested struct fields (`struct Inner` inside `struct Outer`) | Requires recursive layout; defer to v0.3.1 or v0.4 |
| Struct arrays (`Player[4]`) | Combines array + struct layout; follow-on RFC |
| Methods / `impl` / associated functions | Too large for v0.3 |
| `enum`, `union`, tagged unions | Separate type-system milestone |
| Default field values | Syntax not defined |
| Field visibility (`pub`) | Module system not extended |
| Padding / `#[repr(...)]` attributes | Fixed natural layout only in v0.3 |
| Move / copy / drop semantics | All locals remain stack slots with memcpy-style literal init |

If a feature is not listed in §3, assume it is excluded unless a future RFC explicitly adds it.

---

## 5. Compiler Pipeline (unchanged shape)

```text
source (.x)
  → lex
  → parse
  → type check          ← new: struct layout table, field types, literals
  → LLVM IR (Inkwell)   ← new: named struct types, GEP field access
  → Module::verify()
  → clang link (build/run)
```

No new intermediate representations. Structs lower from the typed AST already used in v0.2.

---

## 6. Implementation Milestones

Recommended order:

| Phase | Deliverable |
|-------|-------------|
| **M1 — Layout** | Build struct layout map from `StructDecl`; validate field types; expose offsets |
| **M2 — Frontend** | Parser for struct literals `{ … }` on struct bindings; postfix `.field` |
| **M3 — Type checker** | Struct locals; literal field count/type checks; field read/assign typing |
| **M4 — LLVM backend** | `%StructName = type { … }`; struct alloca; literal stores; GEP load/store |
| **M5 — Tests** | Positive IR snapshots; negative diagnostic tests per RFC-0013 § v0.3 |
| **M6 — Docs** | `docs/releases/v0.3.md`, spec updates, `examples/v0.3/` |

Each phase should keep `cargo test` green before proceeding.

---

## 7. Documentation Deliverables

| Artifact | Purpose |
|----------|---------|
| RFC-0018 (this document) | Scope and roadmap |
| RFC-0019 | Struct layout and declarations |
| RFC-0020 | Struct literals |
| RFC-0021 | Field access and assignment |
| RFC-0022 | LLVM struct lowering |
| RFC-0006 update | LLVM lowering rules § v0.3 |
| RFC-0013 update | Diagnostic codes and negative tests § v0.3 |
| `docs/releases/v0.3.md` | Release notes (at implementation time) |
| `docs/spec/v0.3-language-reference.md` | Canonical v0.3 reference (at implementation time) |

---

## 8. Quality Bar

v0.3 inherits v0.2 engineering constraints:

- Deterministic diagnostics with source spans
- `Module::verify()` before IR leaves the compiler
- No C backend / no C-as-IR
- LLVM IR snapshot tests with pinned target triple
- Clippy clean, `rustfmt` clean
- Every new v0.3 diagnostic has at least one negative test asserting code, message fragment, and span line

---

## 9. Success Criteria

v0.3 is complete when:

1. All §3 features are implemented in `compiler/`
2. `examples/v0.3/` programs compile and run under `x run`
3. Negative tests cover every v0.3 diagnostic listed in RFC-0013 § v0.3
4. RFC-0006, RFC-0022, and RFC-0013 reflect the shipped behavior
5. LSP reports struct-related syntax/type errors where feasible

---

## 10. Open Questions

1. Should struct literals use **positional** fields only (`{ 1, 2 }`) or also **named** fields (`{ .x = 1, .y = 2 }`) in v0.3?
2. Should v0.3 allow **uninitialized** struct locals (`Vec2 p;`) or require struct literals only?
3. When should struct names become legal in **function signatures** — v0.3 or a follow-on ABI RFC?
4. Should field access chain (`a.b.c`) be supported in v0.3 if nested structs are excluded?

**Proposed defaults for v0.3:** positional literals only; initializer required; struct locals only (no struct params); single-level field access on struct locals.
