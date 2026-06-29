# RFC-0010: Statements and Blocks

## Status

Draft

## Summary

This RFC defines executable statements and blocks for the XLang v0.1 MVP.

Executable statements are semicolon-terminated. Blocks are brace-delimited.

---

## 1. Blocks

```ebnf
block = "{", { statement }, "}" ;
```

Blocks are used for function bodies and `if` branches.

---

## 2. Statements

```ebnf
statement       = binding_stmt
                | assignment_stmt
                | return_stmt
                | if_stmt
                | expr_stmt ;
binding_stmt    = [ "const" ], type_name, identifier, "=", expr, ";" ;
assignment_stmt = identifier, "=", expr, ";" ;
return_stmt     = "return", [ expr ], ";" ;
if_stmt         = "if", expr, block, [ "else", block ] ;
expr_stmt       = expr, ";" ;
```

Every executable statement ends with `;` except `if`, whose branch blocks delimit the statement.

---

## 3. Return Rules

`return expr;` must match the enclosing function return type.

`return;` is valid only in `void` functions.

Non-`void` functions are rejected if control may reach the end of the body without returning a value.

---

## 4. If Statements

`if` conditions must have type `bool`.

`if` is a statement, not an expression, in the MVP. `else if` syntax is postponed; use a nested `if` inside an `else` block when needed.

---

## 5. MVP Scope Rule

The MVP uses one binding namespace per function. Parameters and local declarations share that namespace.

Branch-local declarations reserve their names in the function namespace, but they are not visible after the `if` statement. Assignments to a predeclared mutable local are allowed inside branches.

This conservative rule is temporary. Full nested lexical scopes and definite assignment require later RFCs.

---

## 6. Open Questions

1. When should nested lexical scopes become observable?
2. Should `if` become an expression?
3. Which loop statement enters the MVP next?

**Update (v0.2 — RFC-0015):** `while` with `break` and `continue` is the next loop form. `for` remains excluded.
