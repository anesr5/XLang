# RFC-0039: Enum Types in Function Signatures

## Status

Draft

## Summary

This RFC specifies type-checking rules for **enum types in function parameters and return types**.

---

## 1. Allowed signature types (v0.6)

| Type | Params | Returns |
|------|:------:|:-------:|
| `i32`, `bool`, `void` | yes | yes |
| `str` | yes (frontend) | yes (frontend) |
| Local enum `E` | yes | yes |
| Qualified enum `mod.E` | yes | yes |
| Struct, array | no | no |

---

## 2. `main` restriction

`main` remains **`i32 main()`** with no parameters. Enum returns from `main` are rejected.

---

## 3. Returns

When a function declares return type `E` (enum):

- `return Ok(1);` — constructor call, checked via existing enum constructor rules
- `return expr;` — `expr` must have type `E`

---

## 4. Parameters

Enum parameters bind as **immutable by default** (same as scalars). The callee receives a by-value enum; field-style mutation is not supported.

---

## 5. Call sites

- `f()` where `f` returns `E` — expression type is `E`
- `E x = f();` — annotation must match return type
- `match f() { … }` — scrutinee type is `E`

---

## 6. Disambiguation

`TypeName::Named` and `Qualified` refer to **enum** if the name resolves in the enum table, else **struct** (structs remain blocked in signatures).

---

## 7. Errors

| Condition | Message (substring) |
|-----------|---------------------|
| Struct in signature | `not supported in function signatures yet` |
| Unknown enum | `unknown enum type` |
| Return mismatch | `return type mismatch` |
