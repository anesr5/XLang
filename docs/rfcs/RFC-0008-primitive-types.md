# RFC-0008: Primitive Types

## Status

Draft

## Summary

This RFC defines the primitive type surface for the XLang v0.1 MVP.

The frontend recognizes:

```text
i32
bool
str
void
```

The direct LLVM backend currently lowers only:

```text
i32
bool
void
```

`str` is frontend-recognized and type-checked for early syntax and diagnostic work, but it is rejected by LLVM lowering until string representation and ABI rules are specified.

---

## 1. `i32`

`i32` is the MVP signed integer type.

Integer literals are checked against the signed 32-bit range. Positive literals must fit in `i32::MAX`. The unary spelling `-2147483648` is accepted as the minimum `i32` value.

LLVM lowering maps `i32` to LLVM `i32`.

---

## 2. `bool`

`bool` has the values:

```xlang
true
false
```

Boolean values are required for `if` conditions and logical operators.

LLVM lowering maps `bool` to LLVM `i1`.

---

## 3. `void`

`void` is valid only as a function return type.

It is not a first-class value type. It cannot be used for parameters, local variables, returned expression values, or call arguments.

Void-returning calls are valid as expression statements:

```xlang
void tick() {
    return;
}

i32 main() {
    tick();
    return 0;
}
```

---

## 4. `str`

`str` is reserved as the MVP string type spelling and string literals lex and type-check as `str`.

The LLVM backend rejects `str` because the language has not yet specified string layout, ownership, encoding, or calling convention.

---

## 5. Named Types

Named types are parsed to support future struct work. In v0.1, named types are not accepted in function signatures and are not lowered by the LLVM backend.

Struct layout, construction, field access, and ABI rules require later RFCs.

Named local value declarations are rejected with a type diagnostic until struct values are supported.

---

## 6. Open Questions

1. Which integer widths should follow `i32`?
2. Should `str` be a slice, owned value, pointer pair, or standard-library type alias?
3. Should `char` become a Unicode scalar value, byte, or remain postponed?
4. How should named local type errors be worded once struct values are introduced?
