# Compilation

How source becomes a native executable with the v0.1 toolchain.

## Commands

| Command | Reads | Writes | Executes |
|---------|-------|--------|----------|
| `x check file.x` | source | — | lex → parse → typecheck |
| `x emit-llvm file.x` | source | IR to stdout | full pipeline through verified IR |
| `x build file.x` | source | `build/main.ll`, native binary | emit + `clang` link |
| `x run file.x` | source | same as build | build + run binary |

Default file if omitted: `main.x`

### Target triple

```bash
x emit-llvm examples/main.x --target x86_64-pc-windows-msvc
x build examples/main.x --target wasm32-unknown-unknown
```

- CLI: `--target <triple>` or `--target=<triple>` (before or after file path)
- Environment: `XLANG_TARGET_TRIPLE`

When unset, the backend sets a known host triple on supported platforms:

| Host | Default triple |
|------|----------------|
| Windows x64 (MSVC) | `x86_64-pc-windows-msvc` |
| Linux x64 | `x86_64-pc-linux-gnu` |
| macOS x64 | `x86_64-apple-macosx` |
| macOS arm64 | `arm64-apple-macosx` |

Other hosts may emit IR without an explicit triple.

## Prerequisites

| Component | Purpose |
|-----------|---------|
| Rust (edition 2024) | Build compiler crate `x` |
| LLVM 22 dev libraries | Inkwell / `llvm-sys` link time |
| `clang` on PATH | `build` and `run` native linking |

Windows: set `LLVM_HOME` if LLVM is not at `C:\Program Files\LLVM`.

## LLVM lowering (supported subset)

Module name: `xlang`

Lowering order:

1. Reject unsupported types and expressions in the AST
2. Declare all functions in the LLVM module
3. Emit function bodies
4. Run `Module::verify()`
5. Print or write textual IR

### Type mapping

| XLang | LLVM |
|-------|------|
| `i32` | `i32` |
| `bool` | `i1` |
| `void` | `void` |

### Locals and parameters

Conservative stack-slot lowering:

```text
parameter  → alloca + store argument
local init → alloca + store value
read       → load
assign     → store
```

Parameter and local names are preserved in IR where possible.

### Control flow

- `if` → `if.then`, `if.else`, `if.end` basic blocks
- `&&` / `||` → short-circuit blocks with `i1` phi nodes
- `return` → terminates current block

### Integer operations

Signed operations: `add`, `sub`, `mul`, `sdiv`, `srem`, `icmp` with signed predicates.

### Verification gate

If `Module::verify()` fails, the compiler reports:

```text
LLVM verifier failed: …
```

No IR is printed, written, or linked after verifier failure.

## Build artifacts

| Path | Description |
|------|-------------|
| `build/main.ll` | Verified LLVM IR (text) |
| `build/main.exe` | Native executable (Windows) |
| `build/main` | Native executable (Unix) |

Artifacts are written relative to the **current working directory**, not relative to the source file.

Link command (when `clang` is available):

```bash
clang -Wno-override-module build/main.ll -o build/main.exe
# with optional: -target <triple>
```

If `clang` is missing, `build` writes IR and reports that linking is unavailable.

## Process exit codes (tooling)

| Outcome | `x check` / `x build` / `x emit-llvm` | `x run` |
|---------|---------------------------------------|---------|
| Success | 0 | program's `main` return value |
| Compile error | 1 | 1 (build failed) |
| Usage error | 2 | 2 |

## What the backend rejects

Even if `check` passed, codegen fails on:

- `str` types or string literals
- Named struct types in signatures
- Any construct not mapped in the LLVM emitter

Error:

```text
LLVM MVP supports i32, bool, and void code generation only
```

Some internal LLVM builder failures report location `(1, 1)` instead of a source span.

## Not part of the toolchain today

- Formatter (`x format`)
- Test runner (`x test`)
- Package manager
- Incremental compilation
- Debugger integration
- Stable/official LSP release
- Direct object file emission (uses textual IR + `clang`)
