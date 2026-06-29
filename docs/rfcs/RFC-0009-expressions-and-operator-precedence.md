# RFC-0009: Expressions and Operator Precedence

## Status

Draft

## Summary

This RFC defines the v0.1 MVP expression grammar and operator precedence.

Expressions are deterministic, precedence-based, and semantically checked before LLVM lowering.

---

## 1. Expression Grammar

```ebnf
expr        = logical_or ;
logical_or  = logical_and, { "||", logical_and } ;
logical_and = equality, { "&&", equality } ;
equality    = comparison, { ( "==" | "!=" ), comparison } ;
comparison  = term, { ( "<" | "<=" | ">" | ">=" ), term } ;
term        = factor, { ( "+" | "-" ), factor } ;
factor      = unary, { ( "*" | "/" | "%" ), unary } ;
unary       = ( "-" | "!" ), unary | primary ;
primary     = integer_literal
            | string_literal
            | "true"
            | "false"
            | identifier_or_call
            | "(", expr, ")" ;
```

Binary operators are left-associative. Unary operators bind tighter than binary operators. Parentheses override precedence.

---

## 2. Type Rules

Arithmetic operators require `i32` operands and produce `i32`.

Literal zero divisors and remainders are rejected during type checking for `/` and `%`. Full constant evaluation is postponed.

Comparison operators require `i32` operands and produce `bool`.

Equality operators require both operands to have the same type and produce `bool`.

Logical `&&` and `||` require `bool` operands and produce `bool`.

Unary `-` requires `i32`. Unary `!` requires `bool`.

---

## 3. Calls

Function call syntax is:

```xlang
add(40, 2)
```

The callee must resolve to a known function. Argument count and argument types must match the function signature exactly.

Calls returning `void` may be used only where no value is consumed.

---

## 4. LLVM Lowering

Arithmetic, comparison, equality, unary integer negation, and boolean not lower directly to LLVM integer operations.

Logical `&&` and `||` short-circuit. The right-hand side is lowered into a separate basic block and is evaluated only when required.

---

## 5. Open Questions

1. Should assignment become an expression later?
2. Should casts use function-call syntax, generic syntax, or a dedicated operator?
3. When should bitwise operators become part of the typed expression grammar?
