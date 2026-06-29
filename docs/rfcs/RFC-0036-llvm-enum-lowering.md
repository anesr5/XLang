# RFC-0036: LLVM Enum Lowering

## Status

Draft

## Summary

This RFC defines LLVM lowering for v0.5 **enum locals**, **constructors**, and **`match`**.

---

## 1. Representation

Each enum `E` lowers to a named LLVM struct:

```llvm
%E.tagged = type { i32, i32 }
```

- Field 0: **tag** — variant index `0..n-1` in declaration order
- Field 1: **payload** — `i32`; `0` for unit variants; bool payloads zero-extended to i32

Enum name in IR: `%module.EnumName.tagged` (extends v0.4 module prefix).

---

## 2. Constructors

`None()` for tag 1:

```llvm
store { i32 1, i32 0 }, ptr %slot
```

`Some(42)` for tag 0:

```llvm
store { i32 0, i32 42 }, ptr %slot
```

---

## 3. Match

Lower to **switch** on tag:

```llvm
%tag = extractvalue { i32, i32 } %loaded, 0
switch i32 %tag, label %default [
  i32 0, label %arm_some
  i32 1, label %arm_none
]
```

Payload binding in `Some(v)`:

```llvm
%payload = extractvalue { i32, i32 } %loaded, 1
; bind v in alloca
```

---

## 4. Restrictions

- Enum as function param/return: rejected at codegen (same as struct v0.3)
- Match result type must be scalar supported by backend (`i32`, `bool`)

---

## 5. Verification

Each module passes `Module::verify()` after enum additions.
