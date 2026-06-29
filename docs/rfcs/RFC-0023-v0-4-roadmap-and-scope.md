# RFC-0023: v0.4 Roadmap and Scope

## Status

Draft

## Summary

This RFC defines the scope, goals, and delivery plan for **XLang v0.4** — the first **multi-file module system** milestone after v0.3 struct values.

v0.4 turns `module` and `import` from syntax-only declarations into a working **name resolution and compilation model** across multiple `.x` source files, with **public/private visibility**, **qualified names** (`math.add`), and **LLVM symbol naming** suitable for linking multiple translation units.

v0.4 deliberately excludes package managers, remote dependencies, circular imports, generics, ownership, traits, stdlib, and GPU.

---

## 1. Motivation

v0.1 through v0.3 established:

- A single-file bootstrap compiler with LLVM lowering
- Deterministic diagnostics and IR verification gates
- Struct values usable within one translation unit

Real programs split code across files. RFC-0012 reserved `module` / `import` syntax but explicitly deferred file loading, name resolution, and cross-module calls.

v0.4 closes that gap while keeping the toolchain small: **local filesystem modules only**, no network, no package registry.

---

## 2. v0.4 Goals

The v0.4 compiler should:

1. Treat `module` declarations as **real module identity**, not comments
2. Resolve `import` statements to **on-disk module sources** using a deterministic layout rule
3. Compile **multiple `.x` files** into one executable through separate LLVM modules + link
4. Support **qualified names** (`module.symbol`) and **unqualified imports** where unambiguous
5. Enforce **`pub` visibility** for cross-module functions and struct types
6. Allow **cross-module function calls** and **cross-module struct type usage** (building on v0.3 struct stability)
7. Reject **duplicate modules**, **duplicate exported symbols**, and **circular import graphs**
8. Expand diagnostics and negative tests per [RFC-0030](RFC-0030-v0-4-diagnostics.md)

---

## 3. In Scope (v0.4)

| Feature | RFC |
|---------|-----|
| Module identity and file layout | [RFC-0024](RFC-0024-module-identity-and-source-layout.md) |
| Import resolution | [RFC-0025](RFC-0025-import-resolution.md) |
| Cross-module name resolution | [RFC-0026](RFC-0026-cross-module-name-resolution.md) |
| `pub` / private visibility | [RFC-0027](RFC-0027-public-and-private-symbols.md) |
| Multi-file compilation driver | [RFC-0028](RFC-0028-multi-file-compilation.md) |
| LLVM linking and symbol names | [RFC-0029](RFC-0029-llvm-linking-and-symbol-names.md) |
| Diagnostics and negative tests | [RFC-0030](RFC-0030-v0-4-diagnostics.md) |

### Syntax additions (summary)

```xlang
// math.x
module math

pub i32 add(i32 a, i32 b) {
    return a + b;
}

i32 helper() {
    return 0;   // private to module math
}

// main.x
module main
import math

i32 main() {
    return math.add(40, 2);
}
```

Cross-module struct usage (when v0.3 structs are stable):

```xlang
// geom.x
module geom

pub struct Vec2 {
    i32 x;
    i32 y;
}

// main.x
import geom

i32 main() {
    geom.Vec2 p = { 1, 2 };
    return p.x + p.y;
}
```

---

## 4. Explicit Non-Goals (v0.4)

| Excluded | Reason |
|----------|--------|
| Package manager | Tooling milestone; out of compiler core |
| Remote dependencies | Requires registry, versioning, lockfiles |
| Circular imports | Complicates dependency ordering; reject in v0.4 |
| `import a.b.c` path segments beyond one module name | Flat module namespace first |
| Import aliases (`import math as m`) | Defer to v0.4.1 unless trivial |
| Re-exports (`pub import`) | Defer |
| Generics | Separate type-system milestone |
| Ownership / borrowing | Post-module semantic layer |
| Traits / `impl` | Too large for v0.4 |
| Standard library | Requires module tree + policy |
| GPU / async | Non-CPU backend |
| `str` ABI across modules | String lowering still unspecified |

If a feature is not listed in §3, assume it is excluded unless a future RFC explicitly adds it.

---

## 5. Compiler Pipeline (v0.4)

```text
entry file (.x)
  → discover module graph (imports, transitive)
  → load + parse each module file
  → per-module AST validation
  → global symbol table (modules, pub items)
  → type check whole program (cross-module)
  → emit LLVM IR per module (mangled symbols)
  → verify each module
  → clang link all .o / .ll → executable
```

The v0.1–v0.3 **single-file path remains** as a degenerate case: one module, zero imports.

---

## 6. Implementation Milestones

| Phase | Deliverable |
|-------|-------------|
| **M1 — Layout** | Module file discovery; duplicate module detection |
| **M2 — Imports** | Resolve `import name` to file; reject missing / circular |
| **M3 — Visibility** | `pub` on functions and structs; private by default |
| **M4 — Names** | Qualified `mod.item`; import brings module into scope |
| **M5 — Typecheck** | Cross-module calls; cross-module struct types in signatures/locals |
| **M6 — LLVM** | Per-module IR files; mangled symbol names; link step |
| **M7 — Tests** | Multi-file fixtures; negative tests per RFC-0030 |
| **M8 — Docs** | `docs/releases/v0.4.md`, `examples/v0.4/`, LSP updates (stretch) |

Each phase should keep `cargo test` green before proceeding.

---

## 7. Documentation Deliverables

| Artifact | Purpose |
|----------|---------|
| RFC-0023 (this document) | Scope and roadmap |
| RFC-0024 – RFC-0029 | Module system design |
| RFC-0030 | v0.4 diagnostics |
| RFC-0012 update | Mark syntax-only semantics superseded by v0.4 |
| RFC-0006 update | LLVM § v0.4 linking |
| `docs/releases/v0.4.md` | Release notes (at implementation time) |
| `examples/v0.4/` | Multi-file sample projects |

---

## 8. Quality Bar

v0.4 inherits prior engineering constraints:

- Deterministic diagnostics with source spans (including cross-file)
- `Module::verify()` per emitted LLVM module
- No C-as-IR
- Every v0.4 diagnostic in RFC-0030 has at least one negative test
- Module discovery order must be deterministic (sorted by module name)

---

## 9. Success Criteria

v0.4 is complete when:

1. All §3 features are implemented in `compiler/`
2. `examples/v0.4/` multi-file programs build and run under `x run`
3. Negative tests cover every RFC-0030 diagnostic
4. Cross-module struct usage works for scalar-field structs from v0.3
5. Circular imports and duplicate modules are rejected with stable errors

---

## 10. Open Questions

1. Should module names use dots (`core.math`) in v0.4 or flat identifiers only? **Proposed: flat identifiers.**
2. Should `import math` also inject unqualified `add` when unique? **Proposed: no — qualified names only in v0.4.**
3. Root module file naming: `math.x` vs `math/mod.x`? **Proposed: `math.x` at project root (see RFC-0024).**
4. Should the CLI accept multiple entry files? **Proposed: single entry; transitive imports only.**
