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
cargo run --manifest-path compiler/Cargo.toml -- run examples/main.x
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

fn add(a: i32, b: i32) -> i32 {
    return a + b;
}

fn main() -> i32 {
    let x = add(40, 2);
    return x;
}
```

| Feature | Frontend | LLVM backend |
|---------|:--------:|:------------:|
| `i32`, `bool`, `void` | yes | yes |
| Functions, calls, `return` | yes | yes |
| `let` / `var` / `const`, assignments | yes | yes |
| `if` / `else` | yes | yes |
| Arithmetic, comparison, `&&`, `\|\|` | yes | yes |
| `module`, `import` (syntax only) | parsed | — |
| `struct` declarations | parsed | — |
| `str` literals | type-checked | rejected at codegen |

Rules that matter in v0.1:

- Executable statements end with `;` (newlines do not terminate statements).
- `main` must be `fn main() -> i32` with no parameters.
- `let` and `const` are immutable; use `var` for mutable bindings.
- The backend uses direct LLVM lowering through [Inkwell 0.9](https://github.com/TheDan64/inkwell) (LLVM 22.1). There is **no C backend** and no C-as-IR stage.

---

## Project layout

```text
XLang/
├── docs/rfcs/          Language specification (RFC-0001 … RFC-0006)
├── compiler/           Bootstrap compiler (Rust crate `x`)
│   └── src/
│       ├── lexer.rs    Tokenization
│       ├── parser.rs   Recursive descent + Pratt expressions
│       ├── typeck.rs   Semantic analysis
│       ├── backend/    Inkwell LLVM IR lowering
│       └── compile.rs  Pipeline orchestration
├── examples/           Sample programs
│   ├── main.x
│   ├── invalid_missing_semicolon.x
│   └── invalid_immutable_assignment.x
└── build/              Generated IR and binaries (gitignored)
```

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

All RFCs are currently **Draft**.

---

## Development

```bash
# run the test suite (39 unit tests: lexer, parser, typeck, LLVM snapshots, diagnostics)
cargo test --manifest-path compiler/Cargo.toml
```

Engineering constraints for the MVP:

- Deterministic diagnostics with source spans
- `Module::verify()` gate before any IR is printed, written, or linked
- LLVM IR snapshot tests with pinned target triples
- No generated C artifacts in the backend path

On Windows, `compiler/src/llvm_windows_shim.rs` provides stub symbols for LLVM target initialization entry points missing from the official Windows LLVM installer layout.

---

## Roadmap (high level)

1. ~~Minimal grammar and bootstrap compiler~~ (in progress)
2. Struct layout, construction, and field access
3. Module system and imports
4. Ownership, borrowing, and error values
5. Concurrency and GPU support

See [RFC-0003](docs/rfcs/RFC-0003-mvp-compiler-roadmap.md) for the detailed compiler plan.

---

## License

See [LICENSE](LICENSE).
