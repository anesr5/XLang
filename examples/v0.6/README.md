# XLang v0.6 example

Demonstrates **enum types in function signatures** and **cross-module fallible calls**.

- `math.divide` returns `pub enum ResultI32`
- `main` matches on the call result directly

```bash
cargo run --manifest-path compiler/Cargo.toml -- run examples/v0.6/main.x
echo %ERRORLEVEL%   # expect 5 (10 / 2)
```
