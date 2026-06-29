# Statements

## Binding statement

Introduces a new local name.

```xlang
i32 x = expression;
const bool flag = expression;
```

Errors:

- Duplicate name in the same function scope
- Type mismatch between annotation and initializer
- Binding a void expression
- Assigning later to a `const` binding

## Assignment statement

```xlang
identifier = expression;
```

Requirements:

- Name must refer to an existing **mutable** local or parameter
- Expression type must exactly match the binding type
- Assignment to `const` locals is rejected

## Return statement

```xlang
return;
return expression;
```

| Function return type | Form |
|---------------------|------|
| `void` | `return;` only (or implicit fall-through) |
| non-void | `return expression;` required on all exit paths |

Cannot `return` a void call expression; use `return;` for void functions.

Return type of expression must match the enclosing function's return type.

## If statement

```xlang
if condition {
    // then statements
} else {
    // else statements (optional)
}
```

- `condition` must have type `bool`.
- `if` is not an expression — it does not produce a value.
- No semicolon after closing `}` before a following `else` or next statement.
- Empty `else` branch is allowed (`else` omitted entirely).

For definite-return analysis, an `if` counts as returning only when **both** branches exist and **both** unconditionally return.

## Expression statement

```xlang
expression;
```

Typically used for void function calls:

```xlang
void log_value(i32 x) {
    return;
}

i32 main() {
    log_value(42);
    return 0;
}
```

The expression is type-checked but its value is discarded. Non-void expression statements are allowed (e.g. `add(1, 2);`) but unusual.

## Statement coverage matrix

| Statement | Type-checked | Codegen |
|-----------|:------------:|:-------:|
| `type name = expr;` | yes | yes* |
| `const type name = expr;` | yes | yes* |
| `name = expr;` | yes | yes |
| `return;` | yes | yes |
| `return expr;` | yes | yes |
| `if` / `else` | yes | yes |
| `expr;` | yes | yes |

\* Locals using `str` fail at codegen.

## Unreachable code

After a `return` in a block, later statements in that block are skipped during codegen. The type checker does not warn about unreachable code.
