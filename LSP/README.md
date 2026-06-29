# XLang Language Server

Language Server Protocol implementation for [XLang](../README.md) (`.x` files).

The server uses the XLang bootstrap compiler (`../compiler`) for parsing, type checking, and semantic analysis. It tracks the **v0.3 language subset** (see `../docs/releases/v0.3.md`).

## Features

| Feature | Status |
|---------|--------|
| Diagnostics (lex / parse / type) | yes |
| Semantic highlighting | yes |
| Hover (functions, locals, structs, fields, types) | yes |
| Completion (keywords, types, symbols) | yes |
| **Struct field completion** (`p.` → fields) | yes (v0.3) |
| Go to definition | yes |
| Find references (including field reads/writes) | yes |
| Rename | yes |
| Format | not yet (no formatter in compiler) |

## Build the server

Requires Rust and LLVM (same as the main compiler).

```bash
cargo build --manifest-path LSP/Cargo.toml --release
```

Binary: `LSP/target/release/xlang-language-server.exe` (Windows) or `xlang-language-server` (Unix).

## VS Code development

1. Install Node.js and pnpm.
2. Build the language server (see above).
3. Add the server to your `PATH`, or set `SERVER_PATH` in the debug launch config.
4. From `LSP/`:

```bash
pnpm i
pnpm run compile
```

5. Open `LSP/` in VS Code and press **F5** (Launch Client).
6. Open an `.x` file from the parent XLang repository.

### `.vscode/launch.json` hint

Set `SERVER_PATH` to the absolute path of `xlang-language-server` if it is not on `PATH`.

## Supported language surface

Aligned with `docs/releases/v0.3.md` and RFC-0014 through RFC-0022:

- C-style functions and locals (`i32`, `bool`, struct types)
- `if` / `else`, `while` / `break` / `continue`
- Fixed-size arrays `T[N]`, index expressions
- Struct declarations, struct locals, struct literals, field access and assignment
- `module`, `import` (parsed; no cross-file resolution)
- Types: `i32`, `bool`, `void`, `str` (frontend); struct names in local bindings

## Smoke test

```bash
python LSP/scripts/smoke_test.py
```

## License

MIT (inherited from tower-lsp-boilerplate template)
