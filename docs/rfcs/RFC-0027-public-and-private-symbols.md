# RFC-0027: Public and Private Symbols

## Status

Draft

## Summary

This RFC defines **`pub` visibility** for cross-module access in XLang v0.4.

By default, all top-level items are **private to their module**. Only items marked `pub` may be referenced through qualified names from other modules.

---

## 1. Design Principles

- **Private by default** — no implicit export
- **Explicit export** — `pub` keyword on allowed item kinds
- **Module boundary is the visibility unit** — no file-level or block-level `pub` in v0.4
- **Consistent with qualified access** — importers use `module.item`; privacy checks the export table

---

## 2. Syntax

Extend function and struct declarations:

```ebnf
visibility     = [ "pub" ] ;
function_decl  = visibility, type_name, identifier, "(", [ param_list ], ")", block ;
struct_decl    = visibility, "struct", identifier, "{", { field_decl }, "}" ;
```

Examples:

```xlang
pub i32 add(i32 a, i32 b) {
    return a + b;
}

i32 helper() {
    return 0;
}

pub struct Vec2 {
    i32 x;
    i32 y;
}

struct Internal {
    i32 tag;
}
```

Imports, modules, and locals **cannot** be `pub` in v0.4.

---

## 3. Exported Item Kinds

| Item | `pub` allowed | Cross-module use |
|------|:-------------:|------------------|
| Function | yes | `mod.fn(...)` call |
| Struct type | yes | `mod.Struct` in types, locals, params, returns |
| Struct fields | no separate visibility | inherit struct visibility |
| Module | N/A | via `import` only |
| Parameters / locals | no | never exported |

**Decision for v0.4:** struct fields do **not** have individual visibility. Exporting a struct exports all fields for layout purposes; importers use field access syntax on values they own.

---

## 4. Private Items

Private functions and structs are usable **only inside the same module**:

```xlang
// math.x
module math

i32 helper() {
    return 1;
}

pub i32 add(i32 a, i32 b) {
    return a + b + helper();
}
```

From `main.x`:

```xlang
math.helper();   // error: `helper` is private to module `math`
```

---

## 5. `main` and Entry Points

`i32 main()` need **not** be `pub`. The linker entry is resolved by **function name** `main` in the **entry module** only (RFC-0011).

Other modules must not define `main` unless they are the entry module.

**Decision for v0.4:** at most **one** `main` function across the whole program (in entry module).

---

## 6. Type Checking Interaction

When checking `math.add`:

1. Resolve `add` in export table of `math`
2. Verify `pub`
3. Apply existing v0.1 signature rules

When checking `geom.Vec2` type:

1. Resolve struct in export table
2. Verify `pub`
3. Apply v0.3 struct layout rules

Private struct used in **public function signature** of same module — allowed.

Private struct in **pub function signature** — allowed within module; importers never see that function's private types unless exported another way (not in v0.4).

---

## 7. Duplicate and Conflicting Exports

Within a module, duplicate names remain illegal (functions, structs).

There is no `pub` overload set across modules — each exported name is unique **within** its module.

---

## 8. Diagnostics

See [RFC-0030](RFC-0030-v0-4-diagnostics.md):

```text
error[E0200]: `helper` is private to module `math`
error[E0200]: cannot use private struct `Internal` from module `math`
error[E0100]: visibility modifier `pub` is not allowed here
```

---

## 9. Migration from v0.1–v0.3

Single-file programs without cross-module references:

**Decision for v0.4:** existing top-level functions and structs behave as **`pub`** for backward compatibility **only when the module has zero imports and is the sole module in the compilation**.

Once a file `import`s another module, or multiple files compile together, **`pub` is required** on all symbols referenced cross-module.

Alternatively (stricter): **always require `pub` for any cross-module-visible symbol**; single-file zero-import programs treat all items as implicitly pub.

**Proposed default:** implicit pub when compilation unit has exactly one module and no imports; otherwise explicit `pub` required for exports.

---

## 10. Negative Tests (required)

| Test | Assert |
|------|--------|
| Cross-module call to private fn | private symbol |
| Cross-module use of private struct | private symbol |
| `pub` on local binding | parse/type error |
| Two `main` in program | duplicate main |

---

## 11. Open Questions

1. Should `pub` on struct be required even for single-file programs? **Proposed: no (compat).**
2. Field-level `pub` for structs? **Proposed: defer.**
