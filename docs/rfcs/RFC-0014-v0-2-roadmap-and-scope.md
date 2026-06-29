# RFC-0014: v0.2 Roadmap and Scope

## Status

Draft

## Summary

This RFC defines the scope, goals, and delivery plan for **XLang v0.2** — the first control-flow and memory-layout extension after the v0.1 bootstrap MVP.

v0.2 adds **stack-allocated fixed-size arrays**, **indexing with bounds checks**, and **`while` loops with `break` and `continue`**, including LLVM lowering and negative diagnostic tests.

v0.2 deliberately excludes larger language features (generics, ownership, heap, structs-as-values, GPU, async, package management, and `str` codegen).

---

## 1. Motivation

v0.1 established:

- C-style functions and locals
- `if` / `else` and `return`
- Direct LLVM lowering for `i32`, `bool`, and `void`
- Deterministic diagnostics and IR verification gates

Real programs need iteration and contiguous fixed-size storage without heap allocation. v0.2 closes that gap while keeping the compiler small, auditable, and verifier-gated.

---

## 2. v0.2 Goals

The v0.2 compiler should:

1. Parse and type-check `while`, `break`, and `continue`
2. Parse and type-check fixed-size array types, array literals, and index expressions
3. Emit runtime bounds checks for every index read and write
4. Lower loops and stack arrays to LLVM IR through Inkwell
5. Pass `Module::verify()` for all supported programs
6. Expand the test suite with **negative diagnostic tests** for every new error path
7. Update language reference and release documentation for v0.2

---

## 3. In Scope (v0.2)

| Feature | RFC |
|---------|-----|
| `while` loops | [RFC-0015](RFC-0015-while-loops-break-and-continue.md) |
| `break` / `continue` | [RFC-0015](RFC-0015-while-loops-break-and-continue.md) |
| Fixed-size stack arrays | [RFC-0016](RFC-0016-fixed-size-arrays.md) |
| Array literals | [RFC-0016](RFC-0016-fixed-size-arrays.md) |
| Index expressions `arr[i]` | [RFC-0017](RFC-0017-index-expressions-and-bounds-checking.md) |
| Runtime bounds checking | [RFC-0017](RFC-0017-index-expressions-and-bounds-checking.md) |
| LLVM lowering for loops and arrays | [RFC-0006](RFC-0006-llvm-ir-lowering-rules.md) § v0.2 |
| New diagnostics and negative tests | [RFC-0013](RFC-0013-diagnostics-and-error-codes.md) § v0.2 |

### Syntax additions (summary)

```xlang
i32 buf[4] = { 1, 2, 3, 4 };
i32 x = buf[2];

while x > 0 {
    if x == 1 {
        break;
    }
    x = x - 1;
    continue;   // optional; jumps to next condition evaluation
}
```

---

## 4. Explicit Non-Goals (v0.2)

The following are **out of scope** for v0.2. They must not appear in the grammar, type checker, or LLVM backend for this release.

| Excluded | Reason |
|----------|--------|
| `for` loops | Deferred to a later RFC; `while` covers MVP iteration |
| Dynamic arrays / vectors | Requires heap or runtime length model |
| Heap allocation (`new`, `malloc`, allocators) | Ownership and ABI not specified |
| Ownership and borrowing | Postponed until value types and lifetimes are defined |
| Pointers and references | Unsafe surface; not needed for stack arrays |
| Struct implementation (values, fields, layout) | Separate milestone after arrays |
| Generics | Too large for v0.2 |
| Traits / `impl` | Too large for v0.2 |
| GPU / `parallel` / `spawn` | Non-CPU backend |
| Async / `await` | Concurrency model not defined |
| Package manager / multi-crate builds | Tooling milestone |
| `str` in LLVM backend | String ABI still unspecified |

If a feature is not listed in §3, assume it is excluded unless a future RFC explicitly adds it.

---

## 5. Compiler Pipeline (unchanged shape)

```text
source (.x)
  → lex
  → parse
  → type check          ← new: loops, arrays, index bounds typing
  → LLVM IR (Inkwell)   ← new: while/break/continue, array alloca, GEP, checks
  → Module::verify()
  → clang link (build/run)
```

No new intermediate representations. Arrays and loops lower from the typed AST already used in v0.1.

---

## 6. Implementation Milestones

Recommended order:

| Phase | Deliverable |
|-------|-------------|
| **M1 — Frontend** | Lexer keywords active (`while`, `break`, `continue`); parser for loop stmt and array syntax; AST extensions |
| **M2 — Type checker** | Loop context for break/continue; array types; index typing; literal length checks |
| **M3 — Bounds policy** | Canonical runtime check IR pattern; trap/abort path on failure |
| **M4 — LLVM backend** | `while` CFG; `[N x T]` allocas; GEP + guarded load/store |
| **M5 — Tests** | Positive IR snapshots; negative diagnostic tests per RFC-0013 § v0.2 |
| **M6 — Docs** | `docs/releases/v0.2.md`, spec updates, examples under `examples/v0.2/` |

Each phase should keep `cargo test` green before proceeding.

---

## 7. Documentation Deliverables

| Artifact | Purpose |
|----------|---------|
| RFC-0014 (this document) | Scope and roadmap |
| RFC-0015 | Loop statements |
| RFC-0016 | Fixed-size arrays |
| RFC-0017 | Indexing and bounds checks |
| RFC-0006 update | LLVM lowering rules for v0.2 |
| RFC-0013 update | Diagnostic codes and negative tests |
| `docs/releases/v0.2.md` | Release notes (at implementation time) |
| `docs/spec/v0.2-language-reference.md` | Canonical v0.2 reference (at implementation time) |

---

## 8. Quality Bar

v0.2 inherits v0.1 engineering constraints:

- Deterministic diagnostics with source spans
- `Module::verify()` before IR leaves the compiler
- No C backend / no C-as-IR
- LLVM IR snapshot tests with pinned target triple
- Clippy clean, `rustfmt` clean
- Every new `E02xx` / `E03xx` diagnostic has at least one negative test asserting code, message fragment, and span line

---

## 9. Success Criteria

v0.2 is complete when:

1. All §3 features are implemented in `compiler/`
2. `examples/v0.2/` programs compile and run under `x run`
3. Negative tests cover every v0.2 diagnostic listed in RFC-0013 § v0.2
4. RFC-0006 and RFC-0013 reflect the shipped behavior
5. LSP (optional stretch) reports new syntax errors; semantic features may follow in v0.2.1

---

## 10. Open Questions

1. Should v0.2 allow uninitialized array locals (`i32 buf[4];`) or require array literals / element-wise assignment only?
2. Should bounds failure trap (`llvm.trap`), call a runtime `xlang.panic_bounds`, or use a configurable abort hook?
3. Are array-typed function parameters required in v0.2, or locals-only first?
4. Should `continue` be restricted to the innermost `while` only (yes in this RFC) or general labeled loops later?
