# RFC-0034: Match Expressions and Exhaustiveness

## Status

Draft

## Summary

This RFC defines **`match` expressions**: syntax, typing, pattern bindings, and **exhaustiveness** over enum variants.

---

## 1. Syntax

```xlang
match scrutinee {
    Pattern => expr,
    Pattern => { stmts },
}
```

- `match` is an **expression** producing a value
- Scrutinee must have an **enum type**
- Arms separated by commas; trailing comma allowed
- Arm body: single expression **or** block ending with an expression statement (no `return` required in block if last stmt is expr)

```xlang
match x {
    Some(v) => v + 1,
    None => 0,
}
```

Patterns (v0.5):

| Pattern | Meaning |
|---------|---------|
| `Variant` | Unit variant |
| `Variant(name)` | Payload variant; binds payload |
| `_` | Wildcard (optional catch-all) |

---

## 2. Typing

1. All arm bodies must produce the **same type**
2. Match expression type = arm body type
3. Payload bindings are in scope only in the arm body
4. Match may appear in `return`, bindings, and nested expressions

---

## 3. Exhaustiveness

For a scrutinee of enum `E` with variants `{V1..Vn}`:

- Arms must cover **every variant** by name, **or**
- Include wildcard `_` covering remaining variants

**Decision for v0.5:** wildcard allowed; if present, named variants need not all be listed. Without wildcard, **all variants required**.

Duplicate patterns in one match are rejected.

---

## 4. Negative Tests

| Scenario | Diagnostic |
|----------|------------|
| Non-enum scrutinee | expected enum type |
| Missing variant | non-exhaustive match |
| Duplicate pattern | duplicate match arm |
| Arm type mismatch | arm type mismatch |
| Unknown variant in pattern | unknown variant |
