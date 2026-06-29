# Overview

## What XLang is (v0.1)

XLang is a systems programming language in early development. The v0.1 bootstrap compiler is a **CPU-only**, **high-assurance MVP**: a small, deterministic frontend plus a direct LLVM IR backend.

This reference documents only the language surface and semantics that the current compiler implements.

## Source files

| Property | Value |
|----------|-------|
| Encoding | UTF-8 text |
| Extension | `.x` |
| Line endings | LF or CRLF (both accepted) |

The lexer treats a UTF-8 BOM (`U+FEFF`) at the start of a file as an unknown character and reports an error.

## Program shape

A program is a single translation unit containing, in order:

1. An optional `module` declaration
2. Zero or more `import` declarations
3. Zero or more top-level items (`struct` declarations or functions)

There is no separate declaration order requirement among structs and functions beyond source order. All functions are visible to the type checker and codegen regardless of definition order (forward calls are allowed).

## Entry point

Every program must contain exactly one function with this signature:

```xlang
i32 main() {
    // ...
}
```

- Return type must be `i32`.
- Parameter list must be empty.
- `main` is used as the process entry point; the returned `i32` becomes the process exit code when run via the `x run` command.

## Compilation pipeline

```text
source (.x)
  → lex
  → parse
  → type check
  → LLVM IR generation (Inkwell)
  → Module::verify()
  → (optional) clang link → native executable
```

| Stage | Command | Output |
|-------|---------|--------|
| Frontend only | `x check` | Success or diagnostic |
| LLVM IR | `x emit-llvm` | Verified textual IR on stdout |
| Native binary | `x build` | `build/main.ll`, `build/main.exe` (or `build/main`) |
| Run | `x run` | Build + execute; exit code = return value |

There is **no C backend** and no intermediate C generation stage.

## Design constraints (current)

- **Explicit types** in function signatures, parameters, and local declarations.
- **No implicit conversions** between types.
- **No garbage collector**; locals lower to stack slots (`alloca` / `store` / `load`) in the MVP backend.
- **Deterministic diagnostics** with line/column locations for most frontend and many backend errors.
