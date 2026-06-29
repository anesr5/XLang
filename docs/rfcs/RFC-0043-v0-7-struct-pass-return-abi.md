# RFC-0043: v0.7 Struct Pass/Return ABI

## Status

Draft

## Summary

v0.7 should allow declared struct types in function parameters and return types, using direct LLVM struct values for the bootstrap ABI.

v0.3 introduced struct locals and field access. v0.4 introduced module-qualified struct names. v0.6 proved the by-value ABI pattern for enum values. This RFC applies the same discipline to structs while keeping ownership and borrowing out of scope.

---

## 1. Goals

1. Allow local struct types in function parameters and return types.
2. Allow `pub struct` types in cross-module function signatures.
3. Lower struct parameters and returns as named LLVM struct values by value.
4. Preserve existing stack-local struct behavior and field access.
5. Add LLVM IR snapshots for local and cross-module struct ABI.

---

## 2. Non-Goals

| Excluded | Reason |
|----------|--------|
| Ownership and borrowing | Separate semantic milestone |
| Nested structs | Current struct field rules reject nested structs |
| Arrays in structs | Current field rules reject arrays |
| Generic structs | No generic system yet |
| Destructuring bind syntax | Field access remains explicit |
| C ABI compatibility guarantee | Bootstrap ABI is internal XLang-to-XLang ABI |

---

## 3. Type Checking

Function signatures may use:

| Type | Params | Returns |
|------|--------|---------|
| `i32` | yes | yes |
| `bool` | yes | yes |
| `void` | no | yes |
| local `Point` struct | yes | yes |
| qualified `geom.Point` struct | yes, if public/imported | yes, if public/imported |
| enum types | unchanged from v0.6 | unchanged from v0.6 |
| arrays | no | no |
| `str` | frontend-only until string ABI | frontend-only until string ABI |

Private cross-module structs must be rejected with `cannot use private struct`.

---

## 4. LLVM Lowering

Named struct layout remains:

```llvm
%main.Point = type { i32, i32 }
```

Parameter lowering:

```llvm
define i32 @xlang.main.sum(%main.Point %p) {
entry:
  %p.addr = alloca %main.Point
  store %main.Point %p, ptr %p.addr
  ...
}
```

Return lowering:

```llvm
define %main.Point @xlang.main.origin() {
entry:
  ret %main.Point { i32 0, i32 0 }
}
```

Call lowering:

```llvm
%p = call %geom.Point @xlang.geom.origin()
%x = call i32 @xlang.geom.sum(%geom.Point %p)
```

---

## 5. Diagnostics

Required negative tests:

- private cross-module struct in signature
- unknown struct type in signature
- struct return type mismatch
- struct parameter type mismatch
- array or unsupported field shape reaching a struct ABI path

---

## 6. Success Criteria

1. Existing v0.1-v0.6 examples and tests continue passing.
2. Struct params and returns work in same-module programs.
3. `pub struct` params and returns work across modules.
4. LLVM snapshots pin local and cross-module struct ABI.
5. Docs and examples include a v0.7 struct ABI sample.
