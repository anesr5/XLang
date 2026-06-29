# RFC-0038: v0.6 Roadmap and Scope

## Status

Draft

## Summary

This RFC defines **XLang v0.6** — **enum types in function signatures** so fallible and optional values flow across function boundaries and modules.

v0.5 limited enums to **stack locals**. v0.6 enables **`ResultI32` / `OptionI32` (and any declared enum) as parameter and return types**, with **cross-module `pub enum`** support matching v0.4 visibility rules.

---

## 1. Motivation

The RFC-0035 `divide` example requires returning `ResultI32` from a function and matching at the call site. v0.5 could only bind enum constructors locally; v0.6 closes that gap without generics.

---

## 2. v0.6 Goals

1. Allow **enum types in function parameters and return types** (local and `module.Enum`)
2. **`return Ok(x)` / `return Err(code)`** from enum-returning functions
3. **Call sites** bind or match enum return values (`let r = f();`, `match f() { … }`)
4. **`pub enum`** exported across modules with the same rules as `pub struct`
5. **LLVM ABI**: pass and return `{ i32 tag, i32 payload }` **by value** (same layout as v0.5 locals)
6. Diagnostics and negative tests per [RFC-0042](RFC-0042-v0-6-diagnostics.md)

---

## 3. In Scope

| Feature | RFC |
|---------|-----|
| Enum in signatures | [RFC-0039](RFC-0039-enum-types-in-function-signatures.md) |
| Cross-module `pub enum` | [RFC-0040](RFC-0040-cross-module-enum-exports.md) |
| LLVM pass/return ABI | [RFC-0041](RFC-0041-llvm-enum-abi-pass-and-return.md) |
| Diagnostics | [RFC-0042](RFC-0042-v0-6-diagnostics.md) |

### Syntax summary

```xlang
module math

pub enum ResultI32 {
    Ok(i32 value);
    Err(i32 code);
}

pub ResultI32 divide(i32 a, i32 b) {
    if b == 0 {
        return Err(1);
    }
    return Ok(a / b);
}
```

```xlang
module main
import math

i32 main() {
    return match math.divide(10, 2) {
        Ok(v) => v,
        Err(_) => 0,
    };
}
```

---

## 4. Non-Goals (v0.6)

| Excluded | Reason |
|----------|--------|
| Struct params/returns | Separate ABI milestone |
| Generic enums | No generic system |
| Enum params in `main` | `main` stays `i32 main()` |
| `str` in signatures at codegen | String ABI deferred |
| Qualified variant constructors (`math.Ok`) | Use local enum type + import |

---

## 5. Success Criteria

1. All §3 features ship in `compiler/`
2. `examples/v0.6/` builds and runs (exit **5** for `10 / 2`)
3. Negative tests cover RFC-0042
4. v0.5 programs remain valid
