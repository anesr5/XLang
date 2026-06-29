# RFC-0031: v0.5 Roadmap and Scope

## Status

Draft

## Summary

This RFC defines the scope, goals, and delivery plan for **XLang v0.5** — the first **algebraic enum** milestone after v0.4 modules.

v0.5 adds **enum declarations**, **variant constructors**, **`match` expressions** with **exhaustiveness checking**, **Option/Result naming conventions**, and **LLVM tagged-union lowering** for stack enum locals with scalar payloads (`i32`, `bool`).

v0.5 deliberately excludes generics, enum parameters/returns in function signatures, nested enums, cross-module enum ABI beyond v0.4 `pub` rules, methods on enums, and `str` payloads at codegen.

---

## 1. Motivation

v0.1–v0.4 established:

- Scalar types, control flow, arrays, structs, multi-file modules
- Direct LLVM lowering with per-module mangling

Programs need **sum types** for optional values and error handling. v0.5 introduces enums as a minimal tagged-union model without a full generic type system.

---

## 2. v0.5 Goals

The v0.5 compiler should:

1. Parse and validate **enum declarations** with unit and single-field payload variants
2. Support **variant constructors** as calls (`None()`, `Some(42)`)
3. Type-check **`match` expressions** with **exhaustiveness** over known variants
4. Document **Option** and **Result** naming conventions for i32 payloads
5. Lower enum locals to LLVM **`{ i32 tag, i32 payload }`** (bool payloads widened to i32)
6. Expand diagnostics and negative tests per [RFC-0037](RFC-0037-v0-5-diagnostics.md)

---

## 3. In Scope (v0.5)

| Feature | RFC |
|---------|-----|
| Enum declarations | [RFC-0032](RFC-0032-enum-declarations.md) |
| Constructors and payloads | [RFC-0033](RFC-0033-enum-constructors-and-payloads.md) |
| Match and exhaustiveness | [RFC-0034](RFC-0034-match-expressions-and-exhaustiveness.md) |
| Option / Result conventions | [RFC-0035](RFC-0035-option-and-result-conventions.md) |
| LLVM enum lowering | [RFC-0036](RFC-0036-llvm-enum-lowering.md) |
| Diagnostics | [RFC-0037](RFC-0037-v0-5-diagnostics.md) |

### Syntax summary

```xlang
enum OptionI32 {
    Some(i32 value);
    None;
}

i32 main() {
    OptionI32 x = Some(42);
    return match x {
        Some(v) => v,
        None => 0,
    };
}
```

---

## 4. Explicit Non-Goals (v0.5)

| Excluded | Reason |
|----------|--------|
| Generic enums (`Option<T>`) | No generic type system yet |
| Multi-field payloads | Single payload field per variant in v0.5 |
| Enum in function signatures | ABI deferred |
| `str` enum payloads at codegen | String ABI unspecified |
| Nested enums | Layout complexity |
| Enum methods / `impl` | OOP layer deferred |
| `if let` / `while let` | Match-only binding in v0.5 |
| Cross-variant payload type unions | All payloads same width after lowering |

---

## 5. Success Criteria

v0.5 is complete when:

1. All §3 features ship in `compiler/`
2. `examples/v0.5/` programs build and run
3. Negative tests cover RFC-0037 diagnostics
4. Option/Result convention enums type-check and lower correctly

---

## 6. Open Questions

1. Match arm syntax: `=>` expression vs block? **Proposed: expression or block; block must produce value via final expression or `return`.**
2. Wildcard `_` required for non-exhaustive open enums? **Proposed: all variants must be covered; `_` optional catch-all when allowed later.**
