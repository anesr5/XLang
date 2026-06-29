# RFC-0024: Module Identity and Source Layout

## Status

Draft

## Summary

This RFC defines how **module names** map to **source files** on disk for XLang v0.4.

Every compilable `.x` file declares exactly one `module` name. The compiler discovers peer modules through a deterministic **source layout** rooted at the **project directory** of the entry file.

---

## 1. Design Principles

- **One module per file** — a source file contains exactly one `module` declaration
- **Explicit module name** — the `module` identifier is authoritative (filename may differ but is discouraged)
- **Flat module namespace** — module names are single identifiers (`math`, `main`, `geom`), not dotted paths in v0.4
- **Deterministic discovery** — same file tree always yields the same module graph
- **No implicit `main`** — the entry file's `module` name need not be `main`; only `main()` function shape is special

---

## 2. Module Declaration

Syntax (unchanged from RFC-0012):

```ebnf
module_decl = "module", identifier ;
```

Rules:

1. Each `.x` file in the compilation set must contain **exactly one** `module` declaration at the top (after optional leading comments).
2. The declared name must match the **canonical module name** registered in the global module table.
3. **Duplicate module names** across files are a hard error (RFC-0030).

Example:

```xlang
module math

pub i32 add(i32 a, i32 b) {
    return a + b;
}
```

---

## 3. Source Layout (v0.4)

### Project root

The **project root** is the directory containing the **entry file** passed to the CLI:

```bash
x run src/main.x
# project root = directory containing main.x (e.g. ./src if path is src/main.x)
```

**Decision for v0.4:** project root = **parent directory of the entry file**.

### Module file resolution

For a module name `M`, the compiler searches **only** under project root:

| Priority | Path pattern |
|----------|--------------|
| 1 | `{root}/M.x` |
| 2 | `{root}/M/main.x` |

If both exist, that is a **duplicate module layout error**.

If neither exists, `import M` fails with **module not found**.

Examples:

```text
project/
  main.x          module main
  math.x          module math
  geom/
    main.x        module geom   (via geom/main.x)
```

---

## 4. Module Name vs Filename

The compiler uses the **`module` declaration** as the logical name.

If `math.x` declares `module algebra`, the module's logical name is **`algebra`**. The filename is a hint only.

**Recommendation:** require filename to match module name in v0.4 diagnostics as a **warning** (optional stretch) or **error** (stricter).

**Proposed for v0.4:** **error** when declared module name ≠ expected name from file path:

```text
error[E0200]: module name `algebra` does not match file `math.x` (expected `math`)
```

This avoids confusing layouts during bootstrap.

---

## 5. Entry Module

The CLI entry file defines:

- the **entry module** (its `module` declaration)
- the starting point for import graph traversal
- the module that must expose `i32 main()` (function name `main`, not module name)

`i32 main()` remains the process entry symbol policy (RFC-0011); the entry module must contain it.

---

## 6. Single-File Mode

A program with one file and no imports is valid:

```xlang
module main

i32 main() {
    return 0;
}
```

Module discovery still registers `main` as the sole module.

---

## 7. Relationship to RFC-0012

RFC-0012 defined syntax-only modules. v0.4 **supersedes** RFC-0012 §3 semantics:

| RFC-0012 (v0.1) | RFC-0024 (v0.4) |
|-----------------|-----------------|
| Imports have no effect | Imports load modules |
| Duplicate imports preserved | Duplicate imports rejected or deduplicated |
| No file discovery | Deterministic file layout |

Syntax ordering rules (module before imports before items) are unchanged.

---

## 8. Negative Tests (required)

| Scenario | Diagnostic |
|----------|------------|
| Two files declare `module math` | duplicate module |
| `import missing` with no file | module not found |
| Both `M.x` and `M/main.x` exist | ambiguous module layout |
| File without `module` declaration | missing module declaration |
| Module name / path mismatch | module name mismatch |

See [RFC-0030](RFC-0030-v0-4-diagnostics.md).

---

## 9. Open Questions

1. Should nested directories beyond `M/main.x` be supported? **Proposed: no in v0.4.**
2. Should `module` be mandatory in every file? **Proposed: yes.**
3. Should project root be configurable (`--root`)? **Proposed: defer; use entry file parent.**
