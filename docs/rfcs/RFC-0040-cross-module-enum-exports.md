# RFC-0040: Cross-Module Enum Exports

## Status

Draft

## Summary

This RFC extends v0.4 module visibility to **enum types** used in function signatures across modules.

---

## 1. Visibility

- `pub enum E` exports `E` from its module
- Private enums are usable only in their defining module
- Imports do not re-export (same as v0.4)

---

## 2. Qualified types

Importer may write:

```xlang
import math

math.ResultI32 divide(...);
```

Or rely on return type inference at call sites (`math.divide` returns `math.ResultI32`).

---

## 3. Resolution

For `Qualified { module, name }`:

1. Module must be imported (or be the current module)
2. Enum must exist in module exports
3. If cross-module, enum must be `pub`

---

## 4. LLVM symbol names

Enum IR type: `%module.EnumName.tagged` (unchanged from v0.6).

Cross-module functions returning enums use existing `@xlang.module.fn` mangling; enum layout must match across modules (same declaration order).

---

## 5. Non-goals

- Re-export `pub use`
- Cross-module enum **constructors** as `math.Ok(1)` — callers use local constructors only when enum type is in scope via duplicate declaration or future import-of-type syntax

For v0.6, the **defining module** exports the function; the enum type at the call site is inferred from the callee signature.
