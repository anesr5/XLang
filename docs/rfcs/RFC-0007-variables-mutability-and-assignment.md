# RFC-0007: Variables, Mutability, and Assignment

## Status

Draft

## Summary

This RFC defines local variables, mutability, and assignment for the XLang v0.1 MVP.

The MVP uses C-style, type-first local declarations:

```xlang
i32 x = 1;
const i32 limit = 10;
```

Local variables are mutable by default. Prefixing a local declaration with `const` makes that local immutable after initialization. Every local declaration requires an initializer and every executable statement ends with `;`.

---

## 1. Goals

The v0.1 variable model should be:

- explicit about storage type
- easy to parse with one-token and two-token lookahead
- easy to type-check before LLVM lowering
- deterministic for diagnostics and tests
- compatible with stack-slot lowering through Inkwell
- conservative about scope until the language has a full lexical-scope and definite-assignment model

---

## 2. Local Declaration Grammar

Local declarations use the concrete grammar from RFC-0005:

```ebnf
binding_stmt = [ "const" ], type_name, identifier, "=", expr, ";" ;
```

Examples:

```xlang
i32 score = 0;
bool alive = true;
const i32 max_score = 100;
```

Initializer expressions are required in the MVP. Uninitialized locals are postponed because they require definite-assignment rules before use.

There is no `let` or `var` keyword. Mutable locals use `type name = expr;`; immutable locals use `const type name = expr;`.

---

## 3. Mutability

A declaration without `const` creates a mutable local:

```xlang
i32 hp = 100;
hp = hp - 10;
```

A declaration with `const` creates an immutable local:

```xlang
const i32 hp = 100;
```

Assignments to immutable locals are rejected by the frontend:

```xlang
const i32 hp = 100;
hp = 90; // error
```

`const` in this RFC is a local immutability marker. Compile-time constants, item-level constants, and constant evaluation require later RFCs.

---

## 4. Type Checking

The declared local type must match the initializer type exactly.

```xlang
i32 count = 1;      // ok
bool flag = true;   // ok
i32 bad = true;     // error
```

`void` is not a first-class value type and cannot be used for local variables. A call returning `void` may be used only as an expression statement.

```xlang
void tick() {
    return;
}

i32 main() {
    tick();          // ok
    i32 x = tick();  // error
    return 0;
}
```

Named types are parsed for future struct support, but the current semantic and backend MVP accepts only the implemented value types.

---

## 5. Assignment

Assignment syntax is:

```ebnf
assignment_stmt = identifier, "=", expr, ";" ;
```

An assignment is valid only when:

- the target name resolves to a local variable or parameter in the current function
- the target is mutable
- the assigned expression type matches the target type

Example:

```xlang
i32 x = 1;
x = x + 1;
return x;
```

Assignment is a statement in v0.1, not an expression. Chained assignment such as `a = b = 1;` is not part of the MVP.

---

## 6. Scope and Duplicate Names

The MVP uses one binding namespace per function.

Parameters and local declarations share this namespace. A local declaration may not redeclare an existing parameter or local name.

```xlang
i32 add(i32 x) {
    i32 x = 1; // error
    return x;
}
```

Nested lexical scopes are postponed. A local declared inside an `if` branch reserves its name in the function namespace, but branch-local declarations are not usable after the `if`.

```xlang
i32 main() {
    if true {
        i32 x = 1;
    } else {
        i32 x = 2; // error in the MVP
    }
    return 0;
}
```

Assignments to a predeclared mutable local are allowed inside branches:

```xlang
i32 main() {
    i32 x = 0;
    if true {
        x = 1;
    } else {
        x = 2;
    }
    return x;
}
```

---

## 7. LLVM Lowering

The direct LLVM backend lowers parameters and locals to stack slots with Inkwell:

```text
i32 x = value;  -> alloca slot for x, then store value
x = value;      -> store value into x's existing slot
use x           -> load from x's slot
```

`const` has no special LLVM storage form in the MVP. It is enforced by semantic analysis before lowering, so invalid assignments to `const` locals never reach the backend.

The generated LLVM module must still pass `Module::verify()` before IR is printed, written, linked, or executed.

---

## 8. Diagnostics

The parser preserves spans for local declaration identifiers and declared type names. The type checker uses those spans for duplicate-name, type-mismatch, immutable-assignment, unknown-variable, and invalid-`void` diagnostics.

Assignment diagnostics point at the assigned identifier where possible. Initializer diagnostics point at the initializer expression when the expression itself is invalid.

---

## 9. Open Questions

1. Should uninitialized locals be introduced with definite-assignment analysis?
2. Should item-level constants use `const type NAME = expr;`, or a distinct compile-time declaration form?
3. Should future nested lexical scopes allow branch-local declarations with the same name in separate branches?
4. Should mutable locals remain stack-slot lowered before LLVM optimization, or should the frontend eventually emit SSA joins directly?
