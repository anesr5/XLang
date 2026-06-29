# RFC-0026: Cross-Module Name Resolution

## Status

Draft

## Summary

This RFC defines **qualified names** and **cross-module symbol lookup** for XLang v0.4.

Users refer to exported items as `module_name.item_name`. The compiler resolves these against the global module graph built by [RFC-0025](RFC-0025-import-resolution.md).

---

## 1. Qualified Name Syntax

Extend expressions and type names:

```ebnf
qualified_identifier = identifier, ".", identifier ;
```

Uses:

| Context | Example |
|---------|---------|
| Call | `math.add(1, 2)` |
| Type | `geom.Vec2` |
| Struct literal context | `geom.Vec2 p = { 1, 2 };` |

**Decision for v0.4:** exactly **one** dot — `module.item` only. No `a.b.c` chains.

---

## 2. Name Resolution Rules

To resolve `M.N`:

1. `M` must be an **imported module name** visible in the current module's import list
2. Look up symbol `N` in module `M`'s export table
3. `N` must be marked **`pub`** (RFC-0027)
4. Return the symbol kind (function, struct type, etc.)

Failure modes:

| Condition | Diagnostic |
|-----------|------------|
| `M` not imported | `unknown module \`M\`` |
| `N` not in `M` | `\`N\` is not exported from module \`M\`` |
| `N` exists but private | `\`N\` is private to module \`M\`` |
| `N` is wrong kind for use | type error (e.g. call non-function) |

---

## 3. Cross-Module Functions

### Calls

```xlang
import math

i32 main() {
    return math.add(40, 2);
}
```

Type checking uses the **parameter and return types** from `math`'s function declaration.

Forward references within a module remain allowed (v0.1 rule). Cross-module calls require the callee module to be fully parsed before checking the importer.

### Visibility

Only `pub fn` (see RFC-0027) may be called from other modules.

---

## 4. Cross-Module Struct Types

Building on v0.3 struct stability:

### Type names

```xlang
import geom

i32 main() {
    geom.Vec2 p = { 3, 4 };
    return geom.Vec2 p2 = { 1, 1 };  // invalid double binding — example shape only
}
```

`geom.Vec2` in a type position resolves to struct `Vec2` exported from module `geom`.

### Requirements

1. Struct must be declared **`pub struct Vec2`** in `geom`
2. Field types must remain within v0.3 codegen subset (`i32`, `bool`)
3. Struct layout is canonical per module; same struct name in two modules is **distinct types**

### Field access

Field access rules (RFC-0021) unchanged — `p.x` on a local struct value.

Cross-module **field types** in expressions follow v0.3 rules.

---

## 5. Unqualified Names

Within a module, unqualified lookup searches **only the current module**:

1. Locals and parameters
2. Functions in current module (pub or private)
3. Struct types in current module

Cross-module items **must** use qualified names in v0.4.

---

## 6. Name Collisions

### Same item name in two imported modules

Not an error until use:

```xlang
import math
import alt

i32 main() {
    return math.add(1, 2);   // OK — qualified
}
```

Unqualified `add` remains unresolved if ambiguous (no unqualified import injection).

### Same struct name in two modules

`math.Vec2` and `geom.Vec2` are **different types** even if fields match. No structural equivalence in v0.4.

---

## 7. Duplicate Symbol Diagnostics

Within one module:

- Duplicate function names — existing v0.1 error
- Duplicate struct names — existing v0.1 error

Across modules:

- Duplicate module names — RFC-0024
- Duplicate **exported** symbol names are **allowed** across modules (qualified disambiguation)

**LLVM note:** symbol mangling must encode module (RFC-0029).

---

## 8. LSP Implications (informative)

The language server should:

- Resolve `math.add` hover to function in `math.x`
- Go-to-definition on qualified names jumps to defining file
- Completion after `math.` lists pub items of imported module `math`

Stretch goal for v0.4.1 if not in initial v0.4 ship.

---

## 9. Negative Tests (required)

| Test | Assert |
|------|--------|
| Call private cross-module function | private symbol |
| Use struct type without `pub` | private symbol |
| `unknown.add(1,2)` without import | unknown module |
| Wrong arity cross-module call | argument mismatch |

---

## 10. Open Questions

1. Should `pub use`-style re-exports exist? **Proposed: defer.**
2. Cross-module struct **parameters** — in scope if struct is pub? **Proposed: yes in v0.4** (enables real APIs).
