# RFC-0003: High-Assurance MVP Compiler Roadmap

## Status

Draft

## Summary

This RFC defines a realistic roadmap for building the first working XLang compiler under a high-assurance quality bar.

The priority is to build a small, coherent, CPU-only compiler with deterministic diagnostics, Inkwell-backed direct LLVM lowering, and verification gates before adding advanced features such as GPU support, async, package management, or complex generics.

---

## 1. MVP Goal

The first MVP compiler should compile a minimal XLang program through direct LLVM IR into a native executable.

Example target program:

```xlang
module main

i32 add(i32 a, i32 b) {
    return a + b;
}

i32 main() {
    i32 x = add(10, 20);
    return x;
}
```

---

## 2. MVP Non-Goals

The MVP should not include:

- GPU support
- async/await
- package manager
- advanced traits
- macros
- full standard library
- advanced borrow checker
- incremental compilation
- debugger
- LSP
- optimizer beyond backend defaults
- C backend
- C as an intermediate representation

---

## 3. Compiler Pipeline

Initial compiler pipeline:

```text
source code
    ↓
lexer
    ↓
parser
    ↓
AST
    ↓
name resolution
    ↓
type checking
    ↓
HIR or typed AST
    ↓
IR generation
    ↓
LLVM backend
    ↓
native executable
```

---

## 4. Required Backend Strategy

The first backend is LLVM. XLang must not use generated C as the MVP backend or as a C-as-IR shortcut.

The bootstrap implementation uses Inkwell to construct LLVM modules directly, verifies each module with LLVM's verifier API, prints deterministic textual LLVM IR for inspection, and invokes `clang` for native linking when available. The compiler contract remains direct LLVM lowering with no generated C stage.

Required backend properties:

- generated LLVM IR is deterministic for the same source and target
- unsupported language features fail with diagnostics instead of partial lowering
- generated IR passes Inkwell `Module::verify()` before it is printed, written, emitted, or linked
- `build` must use verified IR as its input to native linking
- native builds use LLVM tooling only
- tests cover IR snapshots and runtime behavior

---

## 5. Phase 1: Lexer

Support:

- identifiers
- keywords
- integer literals
- float literals
- string literals
- char literals
- operators
- delimiters
- comments
- source locations

Required output:

```text
Token {
    kind
    lexeme
    line
    column
}
```

---

## 6. Phase 2: Parser

Support:

- module declaration
- imports
- function declarations
- variable declarations
- semicolon-terminated executable statements
- blocks
- return statements
- basic expressions
- function calls
- structs
- basic if statements

Struct parsing in the MVP covers top-level declarations and semicolon-terminated fields. Struct construction, field access, layout, and LLVM lowering are postponed until the type-system and memory-layout RFCs define them.

Initial parser style recommendation:

- recursive descent parser
- Pratt parser for expressions

---

## 7. Phase 3: Type Checker

Support:

- primitive types
- local variables
- function signatures
- return type checking
- binary operator checking
- assignment checking
- no unsafe implicit conversions

Primitive types:

```text
i32
bool
void
str (frontend only; rejected by LLVM lowering)
```

---

## 8. Phase 4: LLVM Code Generation

For the first compiler, generate LLVM IR directly through Inkwell.

Example XLang:

```xlang
i32 add(i32 a, i32 b) {
    return a + b;
}
```

Generated LLVM IR direction:

```llvm
define i32 @add(i32 %a, i32 %b) {
entry:
  %t0 = add i32 %a, %b
  ret i32 %t0
}
```

The generated LLVM IR must pass Inkwell `Module::verify()` before object emission or linking. The textual IR printed by `emit-llvm` is the verified module representation, not hand-written C or hand-written assembly.

---

## 9. Phase 5: CLI

Initial commands:

```bash
x check main.x
x build main.x
x run main.x
x test
x format
```

For MVP, only these are required:

```bash
x check
x emit-llvm
x build
x run
```

`check` validates frontend phases only. `emit-llvm` performs Inkwell backend lowering and prints verified LLVM IR. `build` writes verified LLVM IR and then links a native executable with `clang` when the required LLVM tools are installed.

---

## 10. Phase 6: Tests

The compiler should include tests for:

- lexing
- parsing
- type checking
- diagnostics
- code generation
- invalid programs
- runtime behavior
- LLVM IR snapshots
- Inkwell module verifier results
- no C backend artifacts or `gcc` invocation

Example invalid test:

```xlang
i32 x = 3.14;
```

Expected diagnostic:

```text
error[E0100]: expected expression
```

---

## 11. Implementation Language

The v0.1 compiler implementation language is **Rust**.

Rust is chosen for memory safety in the compiler implementation, strong test tooling, disciplined package management, and a practical path to LLVM integration.

---

## 12. Milestones

### Milestone 0: Repository Setup

- project structure
- test runner
- CI
- formatting
- basic CLI

### Milestone 1: Lexer

- tokenization
- source locations
- comments
- literals

### Milestone 2: Parser

- AST
- functions
- blocks
- expressions
- variables

### Milestone 3: Type Checker

- primitive types
- assignments
- return checks
- function calls

### Milestone 4: Direct LLVM Backend

- emit LLVM IR
- verify generated IR
- compile LLVM IR to native code with LLVM tools
- run executable

### Milestone 5: First Demo

Compile and run:

```xlang
module main

i32 main() {
    i32 x = 40;
    i32 y = 2;
    return x + y;
}
```

Expected exit code:

```text
42
```
