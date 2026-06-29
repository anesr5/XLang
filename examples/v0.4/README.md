# XLang v0.4 examples

Multi-module programs with real `import` resolution, `pub` visibility, and qualified calls.

## Layout

```text
v0.4/
  main.x    module main  — entry point, imports math
  math.x    module math  — exports pub functions
```

## Run

From the repository root:

```bash
cargo run --manifest-path compiler/Cargo.toml -- run examples/v0.4/main.x
echo %ERRORLEVEL%   # Windows — expect 42
echo $?             # Unix — expect 42
```

The driver loads `main.x`, resolves `import math` to `math.x`, type-checks both modules, emits `build/main.ll` and `build/math.ll`, links with `clang`, and runs the binary.

## Notes

- Only `pub` items are visible across modules (`math.helper` is private).
- Cross-module calls use qualified syntax: `math.add(40, 2)`.
- LLVM symbols are mangled as `@xlang.math.add`; entry `main` stays `@main`.
