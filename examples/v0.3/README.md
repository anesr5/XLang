# XLang v0.3 examples

Struct locals, struct literals, field read, and field assignment.

| File | Description | Expected exit |
|------|-------------|---------------|
| `main.x` | `Vec2 {3,4}` → sum fields → returns **7** | 7 |

```bash
cargo run --manifest-path compiler/Cargo.toml -- run examples/v0.3/main.x
```

See [RFC-0018](../../docs/rfcs/RFC-0018-v0-3-roadmap-and-scope.md) through [RFC-0022](../../docs/rfcs/RFC-0022-llvm-struct-lowering.md).
