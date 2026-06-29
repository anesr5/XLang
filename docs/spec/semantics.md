# Semantics

Static semantics enforced by the type checker before LLVM lowering.

## Name resolution

| Name kind | Scope |
|-----------|-------|
| Function | Whole program — any function may call any other |
| Parameter | Enclosing function body |
| Local | Enclosing function body, subject to branch visibility rules below |
| Struct type | Parsed as declarations — values and layout are postponed |

There is no module or import resolution.

## Binding namespace

Each function has a **single flat binding namespace** for parameters and locals:

- Parameter names must be unique.
- Local names must not duplicate parameters or earlier locals.
- A local declared in an `if` branch still **occupies the name** for the rest of the function.
- Locals declared in a branch are **not visible** outside that branch's lexical block for **use**, but their names cannot be redeclared later.

Example — duplicate after branch:

```xlang
i32 main() {
    if true {
        i32 x = 1;
    }
    i32 x = 2;   // error: duplicate binding `x`
    return x;
}
```

## Mutability

| Binding | Introduced by | Assignable |
|---------|---------------|:----------:|
| Parameter | function parameter | yes |
| Mutable local | `type name = …;` | yes |
| Immutable local | `const type name = …;` | no |

## Definite return

Non-void functions must return on all paths.

A function body **definitely returns** if at least one top-level statement definitely returns:

- `return expr;` always returns.
- `if cond { … } else { … }` returns only if **both** branches are non-empty and each definitely returns.

Otherwise:

```text
error: function `name` may exit without returning a value
```

`void` functions may end without an explicit `return`.

Statements after a definitely-returning statement in the same block are rejected as unreachable.

## `main` validation

| Check | Error |
|-------|-------|
| Missing `main` | `program must define main()` |
| Parameters on `main` | `` `main` must not have parameters `` |
| Return type ≠ `i32` | `` `main` must return i32 in the MVP `` |

## Type rules (summary)

### Literals

- Integer → `i32` (range-checked)
- `true` / `false` → `bool`
- String → `str`

### Unary

- `-` on `i32` → `i32`
- `!` on `bool` → `bool`

### Binary arithmetic

- Both operands `i32` → result `i32`

### Binary comparison

- Both operands `i32` → result `bool`

### Binary equality

- Both operands same type (and not void) → `bool`

### Binary logical

- Both operands `bool` → `bool`

### Calls

- Argument types match parameters in order
- Result type is callee return type
- Void results cannot be used as values

### Assignments and bindings

- Expression type must equal target type exactly
- No implicit numeric widening or narrowing

## Control flow and locals (runtime model)

The LLVM backend lowers parameters and locals to **stack slots** (`alloca`). Assignments store into the same slot regardless of which `if` branch ran. This gives stable semantics for mutable locals updated in branches.

There is no SSA phi for `if` statement results because `if` is not an expression.

## Diagnostic format

Frontend errors use this shape:

```text
error[E0200]: <message>
 --> file.x:line:column
```

Diagnostics carry structured family-level error codes:

| Code | Family |
|------|--------|
| `E0001` | lexical errors |
| `E0100` | parse errors |
| `E0200` | type errors |
| `E0300` | backend and LLVM lowering errors |
| `E0400` | filesystem and process I/O errors |
| `E9999` | internal compiler errors |

Messages are deterministic, human-readable strings.

Common messages include:

| Message | Cause |
|---------|-------|
| `unknown variable \`x\`` | Use before declaration or out of branch scope |
| `unknown function \`f\`` | Call to undefined function |
| `duplicate binding \`x\`` | Second local with same name in function |
| `duplicate parameter \`x\`` | Repeated parameter name |
| `duplicate function \`f\`` | Two functions with same name |
| `duplicate struct \`S\`` | Two structs with same name |
| `duplicate field \`x\` in struct \`S\`` | Repeated field name inside one struct |
| `cannot assign to immutable binding \`x\`` | Assignment to `const` local |
| `unreachable statement after return` | Statement follows a definitely-returning statement in the same block |
| `if condition must be bool` | Non-bool condition |
| `arithmetic operators require i32 operands` | Mixed or wrong types |
| `expected \`;\`` | Missing semicolon |

## Programs that type-check but fail codegen

| Feature | `check` | `emit-llvm` |
|---------|:-------:|:-----------:|
| `str` in signatures or locals | may pass | fails |
| `str` literals in expressions | may pass | fails |
| Struct types in signatures | fails | — |
| Only `i32` / `bool` / `void` program | pass | pass |

Always validate with the command matching your goal.
