# RFC-0042: v0.6 Diagnostics

## Status

Draft

## Summary

Diagnostics and required negative tests for v0.6 enum signatures and cross-module exports.

---

## 1. Signatures

| Message (substring) | Span |
|---------------------|------|
| `struct type \`...\` is not supported in function signatures yet` | type |
| `enum type \`...\` is not supported in function signatures yet` | type (reserved; prefer struct message when struct) |
| `unknown enum type` | type |
| `cannot use private enum` | type |
| `unknown module` | type |
| `return type mismatch` | return expr |

---

## 2. `main`

| Message | Span |
|---------|------|
| `\`main\` must return i32` | main |
| `\`main\` must not have parameters` | main |

---

## 3. Required Negative Tests

At least one test per row in sections 1-2 before v0.6 ships.

Coverage notes:

- `cannot use private enum` must cover both direct qualified type use and a public function leaking a private enum in its signature.
- `unknown enum type` must cover function signatures, not only local bindings.
- `unknown module` must cover qualified enum signatures.
- `return type mismatch` must cover enum-returning functions, not only scalar functions.

---

## 4. Backend

| Message | Span |
|---------|------|
| `LLVM MVP supports` (unsupported backend type) | backend |

The checked compiler pipeline rejects undeclared enum signatures before LLVM lowering. Backend coverage for this row may use the existing unsupported-type backend tests unless an internal backend-only test harness is added.
