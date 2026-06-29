# Expressions

## Literals

| Literal | Type | Codegen |
|---------|------|---------|
| Decimal integer | `i32` | yes |
| `true` / `false` | `bool` | yes |
| `"…"` string | `str` | no |

## Variables

Bare identifiers refer to parameters or in-scope locals:

```xlang
return x + y;
```

Out-of-scope or unknown names are type errors.

## Function calls

```xlang
callee(arg1, arg2)
```

- Callee must name a function defined in the same program.
- Argument count and types must match the callee signature exactly.
- Void-returning calls produce no usable value.

## Unary operators

| Operator | Operand type | Result type |
|----------|--------------|-------------|
| `-` | `i32` | `i32` |
| `!` | `bool` | `bool` |

Special case: `-2147483648` is accepted as a single `i32` literal via unary negation of `2147483648`.

## Binary operators

### Precedence (high to low)

| Level | Operators | Associativity |
|-------|-----------|---------------|
| 1 | `*`, `/`, `%` | left |
| 2 | `+`, `-` | left |
| 3 | `<`, `<=`, `>`, `>=` | left |
| 4 | `==`, `!=` | left |
| 5 | `&&` | left, **short-circuit** |
| 6 | `\|\|` | left, **short-circuit** |

### Arithmetic (`i32` only)

| Op | Result |
|----|--------|
| `+` `-` `*` `/` `%` | `i32` |

Division and remainder use **signed** LLVM operations. Literal zero divisors and remainders such as `10 / 0` and `10 % 0` are rejected during type checking. General constant evaluation is not implemented yet, so nonliteral divisors are checked only by their type.

### Comparison (`i32` operands)

| Op | Result |
|----|--------|
| `<` `<=` `>` `>=` | `bool` |

### Equality (same-type operands)

| Op | Operands | Result |
|----|----------|--------|
| `==` `!=` | both `i32`, or both `bool`, or both `str` | `bool` |

`str` equality type-checks but cannot be codegen'd if strings appear in the expression tree.

### Logical (`bool` operands, short-circuit)

| Op | Behavior |
|----|----------|
| `&&` | Evaluate right only if left is true |
| `\|\|` | Evaluate right only if left is false |

Short-circuiting is implemented with conditional branches and phi nodes in LLVM IR.

## Parentheses

```xlang
return (1 + 2) * 3;
```

## Expression forms not supported

These may tokenize but **cannot appear** in expression position:

- Float literals (`3.14`)
- Character literals (`'a'`)
- Indexing, field access, method calls
- Casts, `as`, `match`, `if` expressions
- Bitwise operators (`&`, `|`, `^`, `~`, shifts)
- Struct or array literals
- Lambda / closure syntax

## Example

```xlang
i32 clamp(i32 v, i32 lo, i32 hi) {
    if v < lo {
        return lo;
    }
    if v > hi {
        return hi;
    }
    return v;
}

i32 main() {
    return clamp(10, 0, 5 + 3);
}
```
