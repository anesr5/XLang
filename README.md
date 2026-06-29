# XLang Language Specification

XLang is a modern systems programming language designed for native performance, memory safety, fast compilation, safe concurrency, and progressive GPU integration.

This repository contains the early language specification and RFC documents.

## Project Status

Current version: **v0.1 draft**

The project is currently in the v0.1 high-assurance MVP phase. The repository contains the early language specification plus a bootstrap compiler implementation that is intended to grow under production-quality engineering constraints.

## Core Direction

XLang aims to be:

- Native-performance
- Memory-safe without a garbage collector
- Explicit and predictable
- Cross-platform
- GPU-aware
- Concurrency-friendly
- AI-tooling-friendly
- Easy to parse, format, analyze, and generate

## RFC Index

| RFC | Title | Status |
|---|---|---|
| RFC-0001 | Vision, Philosophy, and Non-Goals | Draft |
| RFC-0002 | Syntax Principles | Draft |
| RFC-0003 | MVP Compiler Roadmap | Draft |
| RFC-0004 | Lexical Grammar | Draft |
| RFC-0005 | Concrete Grammar and EBNF | Draft |
| RFC-0006 | LLVM IR Lowering Rules | Draft |

## MVP Compiler

The MVP compiler lives in `compiler/` and is implemented in Rust with an Inkwell-backed direct LLVM IR backend. The project does not use C as an intermediate representation and does not include a C backend.

Supported commands:

```bash
cargo run --manifest-path compiler/Cargo.toml -- check examples/main.x
cargo run --manifest-path compiler/Cargo.toml -- emit-llvm examples/main.x
cargo run --manifest-path compiler/Cargo.toml -- build examples/main.x
cargo run --manifest-path compiler/Cargo.toml -- run examples/main.x
cargo run --manifest-path compiler/Cargo.toml -- emit-llvm examples/main.x --target x86_64-pc-windows-msvc
```

After installing or copying the built binary as `x`, the intended CLI shape is:

```bash
x check examples/main.x
x emit-llvm examples/main.x
x build examples/main.x
x run examples/main.x
x emit-llvm examples/main.x --target x86_64-pc-windows-msvc
```

`check` validates the frontend phases. `emit-llvm` performs Inkwell backend lowering, verifies the LLVM module with `Module::verify()`, and prints LLVM IR. `build` writes the verified IR to `build/main.ll` and invokes `clang` to produce `build/main.exe`. `run` builds and executes that binary.

The target triple can be configured with `--target <triple>` or `XLANG_TARGET_TRIPLE`. If neither is set, the backend uses a known host-derived target triple where supported.

The current backend is pinned to Inkwell `0.9` with the LLVM `22.1` feature. On Windows, the bootstrap build script looks for LLVM under `LLVM_HOME` or `C:\Program Files\LLVM` and links against the installed LLVM import libraries. The official Windows LLVM package does not expose every target-initialization symbol expected by `llvm-sys`, so the MVP includes a small Windows-only shim for unused target initialization entry points.

RFC-0006 specifies the backend contract for direct LLVM lowering, module verification, LLVM IR snapshot tests, and configurable target triples. The current bootstrap defaults to a known host-derived target triple where supported.

Current MVP backend scope: `i32`, `bool`, `void`, functions, calls, local bindings, assignments, returns, basic expressions, and `if` statements. Top-level struct declarations are parsed with semicolon-terminated fields; struct construction, field access, layout, and LLVM lowering are postponed.

Compiler source is split by phase:

```text
compiler/build.rs
compiler/src/diagnostic.rs
compiler/src/token.rs
compiler/src/lexer.rs
compiler/src/ast.rs
compiler/src/parser.rs
compiler/src/typeck.rs
compiler/src/backend/llvm.rs
compiler/src/llvm_windows_shim.rs
compiler/src/compile.rs
compiler/src/main.rs
```

The first demo program is `examples/main.x`; with LLVM and clang installed, it returns process exit code `42`.

XLang v0.1 requires semicolons after executable statements and expression statements:

```xlang
let x = 40;
return x + 2;
```

## Recommended Development Order

1. Define the language philosophy.
2. Define the syntax rules.
3. Define the minimal grammar.
4. Define primitive types.
5. Define functions, scopes, and modules.
6. Define structs, enums, and pattern matching.
7. Define ownership and borrowing.
8. Define errors as values.
9. Define unsafe code.
10. Build a small high-assurance compiler with deterministic diagnostics, tests, direct LLVM IR generation, and Inkwell module verification gates.
11. Add concurrency.
12. Add GPU support.
