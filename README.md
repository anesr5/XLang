# XLang

XLang is a systems programming language in early development — native performance, memory safety without a garbage collector, explicit semantics, and a path toward safe concurrency and GPU support.

This repository holds the **language specification (RFCs)** and a **bootstrap MVP compiler** that lowers directly to LLVM IR.

**Current milestone:** **v0.5** — algebraic enums, variant constructors, `match`, and LLVM tagged-union lowering.

---

## Quick start

### Prerequisites

- [Rust](https://rustup.rs/) (edition 2024)
- [LLVM 22](https://releases.llvm.org/) with development libraries
- `clang` on `PATH` (required for `build` and `run`)

On Windows, set `LLVM_HOME` if LLVM is not installed at `C:\Program Files\LLVM`. The build script links against the LLVM import libraries in that directory.

### Build and run the demo

**Single-file (v0.1):**

```bash
cargo run --manifest-path compiler/Cargo.toml -- run examples/v0.1/main.x
echo $?    # Linux/macOS — expect 42
echo %ERRORLEVEL%  # Windows — expect 42
```

**Multi-module (v0.4):**

```bash
cargo run --manifest-path compiler/Cargo.toml -- run examples/v0.4/main.x
echo %ERRORLEVEL%  # expect 42 (40 + 2 via math.add)
```

**Enums and match (v0.5):**

```bash
cargo run --manifest-path compiler/Cargo.toml -- run examples/v0.5/main.x
echo %ERRORLEVEL%  # expect 42
```

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
| `emit-llvm` | Lower to LLVM IR, verify modules, print entry IR to stdout |
| `build` | Write verified IR to `build/<module>.ll`, link with `clang` |
| `run` | Build and execute the native binary |

```bash
cargo run --manifest-path compiler/Cargo.toml -- check  examples/v0.4/main.x
cargo run --manifest-path compiler/Cargo.toml -- emit-llvm examples/v0.4/main.x
cargo run --manifest-path compiler/Cargo.toml -- build  examples/v0.4/main.x
cargo run --manifest-path compiler/Cargo.toml -- run    examples/v0.4/main.x
```

**Target triple:** pass `--target <triple>` or set `XLANG_TARGET_TRIPLE`.

**Output paths:** `build/<module>.ll` for each compiled module; `build/main.exe` (or `build/main`) for the linked binary.

---

## Language subset (current)

```xlang
module main
import math

i32 main() {
    return math.add(40, 2);
}
```

```xlang
// math.x
module math

pub i32 add(i32 a, i32 b) {
    return a + b;
}
```

| Feature | Frontend | LLVM backend |
|---------|:--------:|:------------:|
| `i32`, `bool`, `void` | yes | yes |
| Functions, calls, `return` | yes | yes |
| C-style locals, `const`, assignments | yes | yes |
| `if` / `else`, `while`, `break`, `continue` | yes | yes |
| Fixed-size stack arrays | yes | yes |
| Structs, literals, field access / assign | yes | yes |
| `module`, `import`, multi-file | yes | yes |
| `pub`, qualified names (`math.add`) | yes | yes |
| `enum`, constructors, `match` | yes | yes |
| `str` literals | type-checked | rejected at codegen |

Rules that matter:

- Executable statements end with `;`.
- Functions: `return_type name(type param, …) { … }`.
- `main` must be `i32 main()` in the **entry module** only.
- Cross-module symbols require `pub`; imports do not re-export.
- LLVM mangling: `@xlang.<module>.<fn>`, entry `@main` unmangled.

---

## Project layout

```text
XLang/
├── compiler/           Bootstrap compiler (Rust crate `x`)
├── LSP/                Experimental language server + VS Code extension
├── docs/
│   ├── releases/       Release notes (v0.1–v0.5)
│   ├── spec/           Language reference
│   ├── compiler/       Compiler architecture
│   └── rfcs/           Specification drafts
├── examples/
│   ├── v0.1/ … v0.5/   Versioned sample programs
└── build/              Generated IR and binaries (gitignored)
```

**Release notes:** [v0.1](docs/releases/v0.1.md) · [v0.2](docs/releases/v0.2.md) · [v0.3](docs/releases/v0.3.md) · [v0.4](docs/releases/v0.4.md) · [v0.5](docs/releases/v0.5.md)

**Examples:** [v0.4 multi-module](examples/v0.4/) · [v0.5 enums](examples/v0.5/)

---

## Specification (RFCs)

| RFC | Title |
|-----|-------|
| [RFC-0001](docs/rfcs/RFC-0001-vision-philosophy-and-non-goals.md) | Vision, philosophy, and non-goals |
| … | … |
| [RFC-0023](docs/rfcs/RFC-0023-v0-4-roadmap-and-scope.md) | v0.4 roadmap and scope (modules) |
| [RFC-0024](docs/rfcs/RFC-0024-module-identity-and-source-layout.md) | Module identity and source layout |
| [RFC-0025](docs/rfcs/RFC-0025-import-resolution.md) | Import resolution |
| [RFC-0026](docs/rfcs/RFC-0026-cross-module-name-resolution.md) | Cross-module name resolution |
| [RFC-0027](docs/rfcs/RFC-0027-public-and-private-symbols.md) | Public and private symbols |
| [RFC-0028](docs/rfcs/RFC-0028-multi-file-compilation.md) | Multi-file compilation |
| [RFC-0029](docs/rfcs/RFC-0029-llvm-linking-and-symbol-names.md) | LLVM linking and symbol names |
| [RFC-0030](docs/rfcs/RFC-0030-v0-4-diagnostics.md) | v0.4 diagnostics |
| [RFC-0031](docs/rfcs/RFC-0031-v0-5-roadmap-and-scope.md) | v0.5 roadmap and scope (enums) |
| [RFC-0032](docs/rfcs/RFC-0032-enum-declarations.md) | Enum declarations |
| [RFC-0033](docs/rfcs/RFC-0033-enum-constructors-and-payloads.md) | Enum constructors and payloads |
| [RFC-0034](docs/rfcs/RFC-0034-match-expressions-and-exhaustiveness.md) | Match and exhaustiveness |
| [RFC-0035](docs/rfcs/RFC-0035-option-and-result-conventions.md) | Option and Result conventions |
| [RFC-0036](docs/rfcs/RFC-0036-llvm-enum-lowering.md) | LLVM enum lowering |
| [RFC-0037](docs/rfcs/RFC-0037-v0-5-diagnostics.md) | v0.5 diagnostics |

Full RFC index in previous README sections; all RFCs are **Draft** unless noted in release notes.

---

## Development

```bash
cargo test --manifest-path compiler/Cargo.toml
```

Engineering constraints:

- Deterministic diagnostics with source spans
- `Module::verify()` per IR unit before link
- LLVM IR snapshot tests with pinned target triples
- No C backend; direct Inkwell lowering (LLVM 22)

---

## Roadmap (high level)

1. ~~Minimal grammar and bootstrap compiler~~
2. ~~v0.2: loops and fixed-size arrays~~
3. ~~v0.3: structs~~
4. ~~**v0.4:** modules, imports, multi-file, `pub`, qualified names~~
5. ~~**v0.5:** enums, constructors, `match`, tagged-union lowering~~
6. Ownership, borrowing, and error values in function signatures
7. Concurrency and GPU support

See [RFC-0003](docs/rfcs/RFC-0003-mvp-compiler-roadmap.md) for the detailed compiler plan.

---

## License

See [LICENSE](LICENSE).
