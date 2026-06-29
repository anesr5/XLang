# XLang

XLang is a systems programming language in early development — native performance, memory safety without a garbage collector, explicit semantics, and a path toward safe concurrency and GPU support.

This repository holds the **v0.1 draft specification** (RFCs) and a **bootstrap MVP compiler** that lowers directly to LLVM IR.

---

## Quick start

### Prerequisites

- [Rust](https://rustup.rs/) (edition 2024)
- [LLVM 22](https://releases.llvm.org/) with development libraries
- `clang` on `PATH` (required for `build` and `run`)

On Windows, set `LLVM_HOME` if LLVM is not installed at `C:\Program Files\LLVM`. The build script links against the LLVM import libraries in that directory.

### Build and run the demo

```bash
cargo run --manifest-path compiler/Cargo.toml -- run examples/v0.1/main.x
echo $?    # Linux/macOS — expect 42
echo %ERRORLEVEL%  # Windows — expect 42
```

The demo program adds `40 + 2` and returns `42` as the process exit code.

### Install the CLI (optional)

```bash
cargo build --manifest-path compiler/Cargo.toml --release
# copy target/release/x.exe (or x) to a directory on your PATH as `x`
```

---

## Compiler commands

| Command | Description |
|---------|-------------|
| `check` | Lex, parse, and type-check — no codegen |
| `emit-llvm` | Lower to LLVM IR, verify the module, print IR to stdout |
| `build` | Write verified IR to `build/main.ll`, link with `clang` |
| `run` | Build and execute the native binary |

```bash
# via Cargo
cargo run --manifest-path compiler/Cargo.toml -- check  examples/main.x
cargo run --manifest-path compiler/Cargo.toml -- emit-llvm examples/main.x
cargo run --manifest-path compiler/Cargo.toml -- build  examples/main.x
cargo run --manifest-path compiler/Cargo.toml -- run    examples/main.x

# after installing the `x` binary
x check examples/main.x
x emit-llvm examples/main.x --target x86_64-pc-windows-msvc
x build examples/main.x
x run examples/main.x
```

**Target triple:** pass `--target <triple>` or set `XLANG_TARGET_TRIPLE`. When unset, the backend picks a known host triple where supported.

**Output paths:** `build/main.ll` and `build/main.exe` (or `build/main` on Unix).

---

## Language subset (MVP)

What works today:

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

| Feature | Frontend | LLVM backend |
|---------|:--------:|:------------:|
| `i32`, `bool`, `void` | yes | yes |
| Functions, calls, `return` | yes | yes |
| C-style local declarations, `const`, assignments | yes | yes |
| `if` / `else` | yes | yes |
| Arithmetic, comparison, `&&`, `\|\|` | yes | yes |
| `module`, `import` (syntax only) | parsed | — |
| `struct` declarations | parsed | — |
| `str` literals | type-checked | rejected at codegen |

Rules that matter in v0.1:

- Executable statements end with `;` (newlines do not terminate statements).
- Functions use C-style syntax: `return_type name(type param, …) { … }` (e.g. `i32 add(i32 a, i32 b)`).
- `main` must be `i32 main()` with no parameters.
- Local declarations use C-style syntax: `i32 x = 1;` creates a mutable local; `const i32 x = 1;` creates an immutable local.
- The backend uses direct LLVM lowering through [Inkwell 0.9](https://github.com/TheDan64/inkwell) (LLVM 22.1). There is **no C backend** and no C-as-IR stage.

---

## Project layout

```text
XLang/
├── compiler/           Bootstrap compiler (Rust crate `x`)
├── LSP/                Experimental language server + VS Code extension
├── docs/
│   ├── releases/       Release notes (v0.1.md)
│   ├── spec/           Language reference (v0.1-language-reference.md)
│   ├── compiler/       Compiler architecture (v0.1-architecture.md)
│   └── rfcs/           Long-term specification drafts
├── examples/
│   └── v0.1/           Canonical v0.1 sample programs
└── build/              Generated IR and binaries (gitignored)
```

**v0.1 docs:** [release notes](docs/releases/v0.1.md) · [language reference](docs/spec/v0.1-language-reference.md) · [compiler architecture](docs/compiler/v0.1-architecture.md)

**v0.2 docs:** [release notes](docs/releases/v0.2.md) · [RFC-0014–0017](docs/rfcs/RFC-0014-v0-2-roadmap-and-scope.md) · [examples/v0.2/](examples/v0.2/)

**v0.3 docs:** [release notes](docs/releases/v0.3.md) · [RFC-0018–0022](docs/rfcs/RFC-0018-v0-3-roadmap-and-scope.md) · [examples/v0.3/](examples/v0.3/)

---

## Specification (RFCs)

| RFC | Title |
|-----|-------|
| [RFC-0001](docs/rfcs/RFC-0001-vision-philosophy-and-non-goals.md) | Vision, philosophy, and non-goals |
| [RFC-0002](docs/rfcs/RFC-0002-syntax-principles.md) | Syntax principles |
| [RFC-0003](docs/rfcs/RFC-0003-mvp-compiler-roadmap.md) | MVP compiler roadmap |
| [RFC-0004](docs/rfcs/RFC-0004-lexical-grammar.md) | Lexical grammar |
| [RFC-0005](docs/rfcs/RFC-0005-concrete-grammar-ebnf.md) | Concrete grammar (EBNF) |
| [RFC-0006](docs/rfcs/RFC-0006-llvm-ir-lowering-rules.md) | LLVM IR lowering rules |
| [RFC-0007](docs/rfcs/RFC-0007-variables-mutability-and-assignment.md) | Variables, mutability, and assignment |
| [RFC-0008](docs/rfcs/RFC-0008-primitive-types.md) | Primitive types |
| [RFC-0009](docs/rfcs/RFC-0009-expressions-and-operator-precedence.md) | Expressions and operator precedence |
| [RFC-0010](docs/rfcs/RFC-0010-statements-and-blocks.md) | Statements and blocks |
| [RFC-0011](docs/rfcs/RFC-0011-functions-and-calling-conventions.md) | Functions and calling conventions |
| [RFC-0012](docs/rfcs/RFC-0012-modules-and-imports.md) | Modules and imports |
| [RFC-0013](docs/rfcs/RFC-0013-diagnostics-and-error-codes.md) | Diagnostics and error codes |
| [RFC-0014](docs/rfcs/RFC-0014-v0-2-roadmap-and-scope.md) | v0.2 roadmap and scope |
| [RFC-0015](docs/rfcs/RFC-0015-while-loops-break-and-continue.md) | While loops, break, continue |
| [RFC-0016](docs/rfcs/RFC-0016-fixed-size-arrays.md) | Fixed-size stack arrays |
| [RFC-0017](docs/rfcs/RFC-0017-index-expressions-and-bounds-checking.md) | Index expressions and bounds checking |
| [RFC-0018](docs/rfcs/RFC-0018-v0-3-roadmap-and-scope.md) | v0.3 roadmap and scope (structs) |
| [RFC-0019](docs/rfcs/RFC-0019-struct-layout-and-declarations.md) | Struct layout and declarations |
| [RFC-0020](docs/rfcs/RFC-0020-struct-literals-and-construction.md) | Struct literals and construction |
| [RFC-0021](docs/rfcs/RFC-0021-field-access-and-assignment.md) | Field access and assignment |
| [RFC-0022](docs/rfcs/RFC-0022-llvm-struct-lowering.md) | LLVM struct lowering |

All RFCs are currently **Draft**.

---

## Development

```bash
# run the test suite (unit tests: lexer, parser, typeck, LLVM snapshots, diagnostics)
cargo test --manifest-path compiler/Cargo.toml
```

Engineering constraints for the MVP:

- Deterministic diagnostics with source spans
- `Module::verify()` gate before any IR is printed, written, or linked
- LLVM IR snapshot tests with pinned target triples
- No generated C artifacts in the backend path
- CI runs formatting, clippy with warnings denied, and the Rust test suite

On Windows, `compiler/src/llvm_windows_shim.rs` provides stub symbols for LLVM target initialization entry points missing from the official Windows LLVM installer layout.

---

## Roadmap (high level)

1. ~~Minimal grammar and bootstrap compiler~~
2. ~~v0.2: loops and fixed-size arrays~~
3. ~~**v0.3:** struct layout, literals, field access, LLVM lowering~~
4. Module system and imports
5. Ownership, borrowing, and error values
6. Concurrency and GPU support

See [RFC-0003](docs/rfcs/RFC-0003-mvp-compiler-roadmap.md) for the detailed compiler plan.

---

## License

See [LICENSE](LICENSE).
