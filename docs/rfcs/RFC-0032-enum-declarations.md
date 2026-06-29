# RFC-0032: Enum Declarations

## Status

Draft

## Summary

This RFC defines **enum declarations** in XLang v0.5: syntax, visibility, variant rules, and name resolution alongside structs.

---

## 1. Syntax

```ebnf
enum_decl   = visibility, "enum", identifier, "{", { variant_decl }, "}" ;
variant_decl = identifier, [ "(", type_name, identifier, ")", ";" ] ;
```

Examples:

```xlang
enum Color {
    Red;
    Green;
    Blue;
}

pub enum OptionI32 {
    Some(i32 value);
    None;
}
```

Rules:

1. At least **one variant** per enum
2. Variant names unique within the enum
3. Unit variants end with `;` (no parentheses)
4. Payload variants have **exactly one** typed field (`type name`)
5. Payload types: `i32` or `bool` only in v0.5
6. `pub` follows the same rules as structs (RFC-0027)

---

## 2. Name Resolution

- Enum type names share the module namespace with structs and functions (must not collide)
- `TypeName::Named("OptionI32")` resolves to enum if not a struct
- Qualified types `mod.EnumName` follow v0.4 import rules when cross-module

---

## 3. Negative Tests

| Scenario | Diagnostic |
|----------|------------|
| Empty enum | at least one variant |
| Duplicate variant | duplicate variant |
| Unknown payload type | unsupported / unknown type |
| `str` payload | not supported |
| Enum / struct same name | duplicate type name |

See [RFC-0037](RFC-0037-v0-5-diagnostics.md).
