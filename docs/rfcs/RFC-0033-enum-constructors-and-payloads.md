# RFC-0033: Enum Constructors and Payloads

## Status

Draft

## Summary

This RFC defines how **enum variants** are constructed as **call expressions** and how **payload values** are stored.

---

## 1. Constructor Syntax

Variants are constructed with call syntax:

```xlang
None()
Some(42)
Ok(1)
Err(0)
```

Rules:

1. **Unit variants** require `()` — `None()` not bare `None`
2. **Payload variants** take exactly **one** argument matching payload type
3. Constructor calls require **expected enum type** from binding annotation or context:

```xlang
OptionI32 x = Some(42);   // OK
Some(42);                   // error: cannot infer enum type
```

---

## 2. Enum Locals

```xlang
OptionI32 x = Some(42);
const OptionI32 y = None();
```

- Enum locals use the same `type name = expr;` binding form as structs
- Initializer must be a variant constructor for the same enum type
- Enum bindings are **immutable by default** unless `type` binding without `const` (same mutability rules as scalars)

---

## 3. Payload Access

Payload bindings occur in **`match` arms** only in v0.5 (RFC-0034), not via field access.

---

## 4. Negative Tests

| Scenario | Diagnostic |
|----------|------------|
| Wrong arity | constructor arity |
| Wrong payload type | type mismatch |
| Unknown variant | unknown variant |
| Uninferable constructor | cannot infer enum type |
