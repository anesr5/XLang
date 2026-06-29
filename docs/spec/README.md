# XLang Language Reference (v0.1)

Stable reference for **what the XLang compiler implements today**. This document set describes the language as accepted by the bootstrap compiler in `compiler/` (crate `x`, version **0.1.0**). It is not a roadmap.

If behavior is not described here, it is **not supported** in the current release.

---

## Scope summary

| Area | Supported today |
|------|-----------------|
| Types in signatures / codegen | `i32`, `bool`, `void` |
| Types in frontend only | `str`, named struct types (parsed, rejected in signatures) |
| Top-level items | optional `module`, `import`, `struct`, functions |
| Functions | C-style: `return_type name(type param, …) { … }` |
| Locals | `type name = expr;` (mutable), `const type name = expr;` (immutable) |
| Control flow | `if` / `else` (statement only), `return` |
| Expressions | integers, booleans, strings, calls, unary/binary ops |
| Backend | Direct LLVM IR via Inkwell; `i32`, `bool`, `void` only |

---

## Document index

| Document | Contents |
|----------|----------|
| **[v0.1 language reference](v0.1-language-reference.md)** | **Canonical single-document spec for v0.1** |
| [Overview](overview.md) | Goals, file format, compilation model |
| [Lexical structure](lexical.md) | Source text, tokens, literals, comments, keywords |
| [Grammar](grammar.md) | Concrete syntax and EBNF for the implemented subset |
| [Types](types.md) | Type names, restrictions, frontend vs backend |
| [Declarations](declarations.md) | Modules, imports, structs, functions, locals |
| [Statements](statements.md) | Bindings, assignment, return, if, expression statements |
| [Expressions](expressions.md) | Literals, operators, precedence, calls |
| [Semantics](semantics.md) | Type checking, scoping, definite return, `main` rules |
| [Compilation](compilation.md) | Toolchain commands, LLVM lowering, artifacts |
| [Limitations](limitations.md) | Explicit non-features and known gaps |

For editor integration, see the [XLang Language Server](../../LSP/README.md) in `LSP/`.

---

## Minimal program

```xlang
module main

i32 add(i32 a, i32 b) {
    return a + b;
}

i32 main() {
    i32 x = add(40, 2);
    return x;
}
```

Every complete program must define **`i32 main()`** with no parameters. The process exit code is the `i32` returned from `main`.

---

## Quick compatibility notes

- **Semicolons are required** after executable statements. Newlines do not terminate statements.
- **C-style syntax only** — functions (`i32 name(…) { }`), locals (`i32 x = …;`), immutables (`const i32 x = …;`). There is no `fn`, `let`, or `var` keyword.
- **`check` may accept programs that `emit-llvm` rejects** (for example programs using `str`).
- **Struct declarations are parsed** but structs cannot be used in types, expressions, or generated code yet.
