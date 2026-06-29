# RFC-0041: LLVM Enum ABI — Pass and Return

## Status

Draft

## Summary

This RFC defines LLVM lowering for **enum parameters and returns** in v0.6, extending v0.5 local representation.

---

## 1. Representation

Unchanged from [RFC-0036](RFC-0036-llvm-enum-lowering.md):

```llvm
%module.EnumName.tagged = type { i32, i32 }
```

---

## 2. Function signature

```llvm
define %main.ResultI32.tagged @xlang.main.divide(i32 %a, i32 %b)
```

Enum parameters and returns use the **tagged struct type** directly (by value).

---

## 3. Parameter lowering

On entry:

```llvm
%r.addr = alloca %main.ResultI32.tagged
store %main.ResultI32.tagged %r, ptr %r.addr
```

Callee treats the binding like a v0.5 enum local (`scalar_ty = None`).

---

## 4. Return lowering

Constructor or value expression produces a struct value; `ret %tagged`.

---

## 5. Call lowering

```llvm
%result = call %math.ResultI32.tagged @xlang.math.divide(i32 10, i32 2)
```

Call sites may store to an enum local alloca or feed directly into `match` scrutinee lowering.

---

## 6. Match scrutinee

v0.6 allows **any enum-typed expression** as scrutinee (variable, call, nested match). Lowering evaluates to a struct value, extracts tag/payload, and switches.

---

## 7. Restrictions

- Struct pass/return: still rejected
- Enum types referenced in signatures must be declared (or imported module's type used via qualified name in checker only)

---

## 8. Verification

Per-module `Module::verify()` after signature changes.
