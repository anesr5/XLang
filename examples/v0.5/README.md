# XLang v0.5 example

Demonstrates **enum declarations**, **variant constructors**, and **`match`** with exhaustiveness checking.

- `OptionI32` — optional `i32` (`Some` / `None`)
- `ResultI32` — fallible `i32` (`Ok` / `Err`) as a local binding
- Exit code **42** after matching through both enums

```bash
cargo run --manifest-path compiler/Cargo.toml -- run examples/v0.5/main.x
echo %ERRORLEVEL%   # expect 42
```
