# XLang v0.2 Examples

Programs using **while**, **break**, **continue**, fixed-size arrays, and bounds-checked indexing.

```bash
cargo run --manifest-path compiler/Cargo.toml -- run examples/v0.2/main.x
# exit code 10 (1+2+3+4)

cargo run --manifest-path compiler/Cargo.toml -- run examples/v0.2/loops.x
# exit code 1
```

## Syntax

```xlang
i32[4] xs = { 1, 2, 3, 4 };
xs[0] = 10;

while i < 4 {
    if cond {
        break;
    }
    continue;
}
```

See [v0.2 release notes](../docs/releases/v0.2.md).
