# RFC-0011: Functions and Calling Conventions

## Status

Draft

## Summary

This RFC defines MVP function syntax, semantic checks, and LLVM calling behavior.

---

## 1. Function Syntax

```ebnf
function_decl = type_name, identifier, "(", [ param_list ], ")", block ;
param_list    = param, { ",", param } ;
param         = type_name, identifier ;
```

Example:

```xlang
i32 add(i32 a, i32 b) {
    return a + b;
}
```

Functions that do not return a value use `void`.

---

## 2. Entry Point

An executable MVP program must define:

```xlang
i32 main() {
    return 0;
}
```

`main` must have no parameters and must return `i32`.

---

## 3. Name and Signature Rules

Function names must be unique within a source file.

Parameter names must be unique within a function.

Parameter and return types must use supported signature types. `void` is valid as a return type only; `void` parameters are rejected.

---

## 4. Calls

Functions may be called before their textual definition because the compiler declares all functions before checking and lowering bodies.

Arguments are checked by arity and exact type match.

Calls returning `void` may be expression statements. Calls returning values may be used in expressions.

---

## 5. LLVM Lowering

XLang functions lower to LLVM functions with matching names.

Parameters are copied into stack slots at function entry. This keeps parameter assignment and local variable lowering uniform in the MVP:

```xlang
i32 bump(i32 x) {
    x = x + 1;
    return x;
}
```

Void functions that reach the end of the body without an explicit return emit `ret void`.

The MVP uses LLVM's default C calling convention. A stable external ABI is not promised until layout and module RFCs mature.

---

## 6. Open Questions

1. Which calling convention names should the language expose?
2. How should external function declarations be written?
3. When should exported symbols and visibility become part of the grammar?
