# RFC-0025: Import Resolution

## Status

Draft

## Summary

This RFC defines how **`import` declarations** resolve to loaded modules in XLang v0.4.

Import resolution builds a **directed acyclic import graph** over module names, loads source files, and prepares the global compilation unit for cross-module type checking.

---

## 1. Syntax

Unchanged from RFC-0012:

```ebnf
import_decl = "import", identifier ;
```

Example:

```xlang
module main
import math
import geom
```

No semicolon. Imports appear after `module` and before items.

---

## 2. Resolution Algorithm

For each import `import M` in module `C`:

1. Look up module file for name `M` using [RFC-0024](RFC-0024-module-identity-and-source-layout.md)
2. If not found → **module not found** diagnostic at import identifier
3. Parse the target file; register module `M`
4. Recursively resolve imports declared in `M`
5. Add edge `C → M` to the import graph

### Ordering

Modules are loaded in **depth-first order** from the entry module, with **import declarations processed in source order**, and sibling branches ordered by **module name ascending** for determinism.

The type checker sees all modules before checking any function body that may reference cross-module symbols.

---

## 3. Duplicate Imports

**Decision for v0.4:** duplicate `import math` in the same file is rejected:

```text
error[E0200]: duplicate import `math`
```

Duplicate imports across different importers are allowed (each file imports what it needs).

---

## 4. Circular Imports

**Decision for v0.4:** circular import graphs are **rejected** at compile time.

Example:

```text
main.x  → import math
math.x  → import main
```

Diagnostic:

```text
error[E0200]: circular import: main → math → main
```

The diagnostic should print the **cycle** and point at the import that closes the cycle.

Algorithm: during DFS, maintain stack of modules; if `M` is already on stack, report cycle.

---

## 5. Transitive Imports

If `main` imports `math`, and `math` imports `geom`, then:

- `main` may use **`math.*`** pub symbols
- `main` may **not** use `geom.*` directly unless `main` also `import geom`

**Decision for v0.4:** imports are **not re-exported**. No transitive visibility.

```xlang
// main.x — INVALID unless `import geom`
geom.Vec2 p = { 0, 0 };   // error: unknown module `geom`
```

---

## 6. Import Scope

An import brings the **module name** into scope as a **namespace** for qualified access:

```xlang
import math
math.add(1, 2);
```

**Decision for v0.4:** imports do **not** inject unqualified function or type names.

Future RFC may add `use math.add` or selective imports.

---

## 7. Self-Import

`import` of the current module is rejected:

```text
error[E0200]: module `math` cannot import itself
```

---

## 8. Unused Imports

**Decision for v0.4:** unused imports are **allowed** (no warning required). A future lint may flag them.

---

## 9. Parser / AST

No syntax change. AST preserves import list per `Program`:

```rust
Program {
    module: Some("main"),
    imports: vec!["math", "geom"],
    ...
}
```

The driver wraps multiple files in a **`CompilationUnit`** (implementation detail) containing `Vec<ModuleUnit>`.

---

## 10. Negative Tests (required)

| Test | Assert |
|------|--------|
| `import missing` | module not found |
| `import math` twice in one file | duplicate import |
| `main ↔ math` cycle | circular import message |
| Use `geom.X` without `import geom` | unknown module |

---

## 11. Open Questions

1. Should unused imports be warnings in v0.4? **Proposed: no.**
2. Should `import` allow string paths later? **Proposed: identifier module names only.**
