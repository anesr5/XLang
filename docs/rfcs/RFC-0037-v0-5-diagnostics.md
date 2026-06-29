# RFC-0037: v0.5 Diagnostics

## Status

Draft

## Summary

Diagnostics and required negative tests for v0.5 enum and match features.

---

## 1. Enum Declarations

| Code | Message (substring) | Span |
|------|---------------------|------|
| `E0200` | `duplicate enum` | enum name |
| `E0200` | `duplicate variant` | variant name |
| `E0200` | `enum must have at least one variant` | enum name |
| `E0200` | `duplicate type name` | struct/enum collision |
| `E0200` | `payload type \`str\` is not supported` | payload type |

---

## 2. Constructors

| Code | Message | Span |
|------|---------|------|
| `E0200` | `unknown variant` | callee |
| `E0200` | `cannot infer enum type for variant constructor` | call |
| `E0200` | `constructor arity mismatch` | call |
| `E0200` | `enum local requires variant constructor initializer` | binding |

---

## 3. Match

| Code | Message | Span |
|------|---------|------|
| `E0200` | `match scrutinee must be an enum type` | scrutinee |
| `E0200` | `non-exhaustive match` | match |
| `E0200` | `duplicate match arm` | pattern |
| `E0200` | `match arm type mismatch` | arm body |
| `E0200` | `unknown variant in match pattern` | pattern |

---

## 4. Required Negative Tests

At least one test per row in §1–§3 before v0.5 ships.
