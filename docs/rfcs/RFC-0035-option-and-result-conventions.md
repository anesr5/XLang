# RFC-0035: Option and Result Conventions

## Status

Draft

## Summary

This RFC defines **naming and shape conventions** for optional and fallible values in v0.5 **without generics**.

---

## 1. Rationale

Full `Option<T>` / `Result<T, E>` require generics. v0.5 uses **fixed scalar payloads** and **conventional enum names** so libraries and examples align before parametric types exist.

---

## 2. Option Convention

Recommended declaration:

```xlang
enum OptionI32 {
    Some(i32 value);
    None;
}
```

Aliases (documentation only in v0.5):

| Name | Variants | Payload |
|------|----------|---------|
| `OptionI32` | `Some`, `None` | `i32` |

The compiler **does not** treat `OptionI32` specially except optional lint-style validation when enum name starts with `Option` and variants match `Some`/`None`.

---

## 3. Result Convention

```xlang
enum ResultI32 {
    Ok(i32 value);
    Err(i32 code);
}
```

| Name | Variants | Payload |
|------|----------|---------|
| `ResultI32` | `Ok`, `Err` | `i32` |

Both payloads are `i32` in v0.5 (error codes as integers).

---

## 4. Usage Example

```xlang
ResultI32 divide(i32 a, i32 b) {
    if b == 0 {
        return Err(1);
    }
    return Ok(a / b);
}

i32 main() {
    return match divide(10, 2) {
        Ok(v) => v,
        Err(_) => 0,
    };
}
```

---

## 5. Cross-Module

`pub enum OptionI32` exports follow v0.4 visibility. Qualified constructors `mod.Some` are **not** in v0.5 — use local enum type and import module for type name only.

---

## 6. Future

Generics will supersede `OptionI32`-style names with `Option<i32>` per a later RFC.
