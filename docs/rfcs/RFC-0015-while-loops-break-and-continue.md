# RFC-0015: While Loops, Break, and Continue

## Status

Draft

## Summary

This RFC adds **`while` loops**, **`break`**, and **`continue`** to XLang for v0.2.

`for` loops are explicitly excluded (see [RFC-0014](RFC-0014-v0-2-roadmap-and-scope.md)).

---

## 1. Design Principles

- **`while` before `for`** — minimal syntax; condition-driven iteration matches v0.1 `if` semantics
- **Statement form only** — loops are not expressions
- **Explicit control transfer** — `break` exits the innermost loop; `continue` jumps to the next condition evaluation
- **Semicolon rules unchanged** — loop headers are not semicolon-terminated; body is a block

---

## 2. Grammar

Extend RFC-0005 statement production:

```ebnf
statement       = binding_stmt
                | assignment_stmt
                | return_stmt
                | if_stmt
                | while_stmt          /* v0.2 */
                | break_stmt          /* v0.2 */
                | continue_stmt       /* v0.2 */
                | expr_stmt ;

while_stmt      = "while", expr, block ;
break_stmt      = "break", ";" ;
continue_stmt   = "continue", ";" ;
```

### Examples

```xlang
i32 sum_to_n(i32 n) {
    i32 total = 0;
    i32 i = 0;
    while i < n {
        total = total + i;
        i = i + 1;
    }
    return total;
}

i32 first_negative(i32 limit) {
    i32 i = 0;
    while i < limit {
        if i == 5 {
            break;
        }
        i = i + 1;
    }
    return i;
}

void skip_odds() {
    i32 i = 0;
    while i < 10 {
        i = i + 1;
        if i % 2 == 1 {
            continue;
        }
        /* even iterations only */
    }
}
```

---

## 3. Lexical

Activate reserved keywords (already tokenized in v0.1 lexer, unused until v0.2):

| Keyword | Role |
|---------|------|
| `while` | Loop statement |
| `break` | Exit innermost enclosing `while` |
| `continue` | Jump to condition of innermost enclosing `while` |

---

## 4. Typing Rules

### While condition

The condition expression must have type **`bool`**.

```text
error[E0200]: while condition must be bool, got I32
```

### Break and continue context

`break` and `continue` are valid **only inside a `while` loop body** (including nested blocks within that body).

They are invalid:

- at function top level
- inside `if` branches that are not enclosed by a `while`
- inside nested functions (when nested functions exist — N/A in v0.2)

```text
error[E0200]: break outside of loop
error[E0200]: continue outside of loop
```

### Definite return

A function body containing `while` still must definitely return on all paths according to RFC-0010 rules.

A `while` loop whose body always `break`s does **not** count as definite return unless another path returns.

`break` from a loop does not constitute function return.

---

## 5. Scoping

v0.2 keeps the v0.1 **flat function-level binding namespace** for parameters and locals.

Locals declared inside a `while` body block follow the same branch visibility rules as `if`:

- Not visible after the loop for **use**
- Names cannot be redeclared later in the function

Assignments to mutable locals declared before the loop are allowed inside the loop body.

---

## 6. Control-Flow Semantics

### While

1. Evaluate condition.
2. If `false`, exit the loop.
3. Execute body block.
4. Go to step 1.

The condition is evaluated **before** each iteration (including the first). Zero iterations occur when the initial condition is `false`.

### Break

Exits the **innermost** enclosing `while` loop and continues execution immediately after the loop statement.

### Continue

Skips the remainder of the current iteration and jumps to the **next condition evaluation** of the innermost enclosing `while`.

`continue` does not re-run initialization before the loop; only the condition and subsequent body executions are affected.

---

## 7. Interaction with If

`break` and `continue` may appear inside `if` / `else` blocks that are lexically inside a `while` body.

```xlang
while cond {
    if inner {
        break;      // valid — exits while
    }
    if skip {
        continue;   // valid — next while condition
    }
}
```

---

## 8. LLVM Lowering (summary)

Full rules: [RFC-0006 § v0.2](RFC-0006-llvm-ir-lowering-rules.md).

Basic block shape:

```text
while.cond:
  %c = load condition
  br i1 %c, label %while.body, label %while.end

while.body:
  ... body ...
  br label %while.cond

while.end:
  ... following statements ...
```

- `break` → `br label %while.end`
- `continue` → `br label %while.cond`

When loops nest, each loop gets its own `.cond`, `.body`, and `.end` labels; `break`/`continue` target the innermost pair.

The backend must track an active loop stack during emission.

---

## 9. Diagnostics (summary)

See [RFC-0013 § v0.2](RFC-0013-diagnostics-and-error-codes.md).

| Code | Condition |
|------|-----------|
| `E0200` | Non-`bool` while condition |
| `E0200` | `break` outside loop |
| `E0200` | `continue` outside loop |
| `E0100` | Missing `;` after `break` / `continue` |

---

## 10. Negative Tests (required)

| Test | Expected |
|------|----------|
| `while 1 { }` | Type error on condition |
| `break;` at function top level | `break outside of loop` |
| `continue;` at function top level | `continue outside of loop` |
| `break` inside `if` without enclosing `while` | Error |
| Unreachable statement after `break` in same block | Error (existing rule) |

---

## 11. Explicit Non-Goals

- `for`, `foreach`, range loops
- `loop { }` infinite loops (keyword reserved, not enabled)
- Labeled `break 'label` / `continue 'label`
- `while` as expression producing a value
- `do { } while (cond)` postfix loops

---

## 12. Open Questions

1. Should empty `while` bodies be allowed (`while cond { }`)?
2. Should the type checker warn on `while true` with no `break` (infinite loop)?
3. When should labeled loops be introduced?
