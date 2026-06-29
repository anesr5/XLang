# XLang Language Server

Language Server Protocol implementation for [XLang](https://github.com/IWANABETHATGUY/tower-lsp-boilerplate) (`.x` files), based on [tower-lsp-boilerplate](https://github.com/IWANABETHATGUY/tower-lsp-boilerplate).

The server uses the XLang bootstrap compiler (`../compiler`) for parsing, type checking, and semantic analysis. It only exposes features supported by the **current v0.1 language subset** (see `../docs/spec/`).

## Features

| Feature | Status |
|---------|--------|
| Diagnostics (lex / parse / type) | yes |
| Semantic highlighting | yes |
| Hover | yes |
| Completion (keywords, types, symbols) | yes |
| Go to definition | yes |
| Find references | yes |
| Rename | yes |
| Format | not yet (no formatter in compiler) |
| Inlay hints | not included (types are explicit in syntax) |
| Struct field access completion | not yet (structs not usable in expressions) |

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

Aligned with `docs/spec/` and RFC-0005 through RFC-0013:

- C-style functions: `i32 add(i32 a, i32 b) { … }`
- C-style locals: `i32 x = 1;`, `const i32 x = 1;`
- `if` / `else`, `return`, assignments
- `module`, `import` (parsed; no cross-file resolution)
- `struct` declarations (parsed only)
- Types: `i32`, `bool`, `void`, `str` (frontend); codegen not required for LSP

## License

MIT (inherited from tower-lsp-boilerplate template)
