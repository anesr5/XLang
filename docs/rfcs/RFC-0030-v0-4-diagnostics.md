# RFC-0030: v0.4 Diagnostics

## Status

Draft

## Summary

This RFC defines **diagnostics and required negative tests** for XLang v0.4 module system features (RFC-0023 through RFC-0029).

It extends [RFC-0013](RFC-0013-diagnostics-and-error-codes.md) with message conventions, span rules, and test requirements. Family codes remain unchanged unless noted.

---

## 1. Error Code Families (unchanged)

```text
E0001 lexical errors
E0100 parse errors
E0200 type errors
E0300 backend and LLVM lowering errors
E0400 filesystem and process I/O errors
E9999 internal compiler errors
```

v0.4 adds **message-level** conventions within existing families. Subcodes (e.g. `E0201`) may be introduced later; tests assert **family + message substring + span line**.

---

## 2. v0.4 Diagnostic Catalog

### Module identity and layout (RFC-0024)

| Code | Message (substring) | Span target |
|------|---------------------|-------------|
| `E0100` | `expected \`module\` declaration` | file start |
| `E0200` | `duplicate module \`M\`` | second `module` name |
| `E0200` | `module name \`X\` does not match file` | module name |
| `E0400` | `module file not found for \`M\`` | import or driver |
| `E0200` | `ambiguous module layout for \`M\`` | import or driver |

### Import resolution (RFC-0025)

| Code | Message | Span target |
|------|---------|-------------|
| `E0200` | `module not found: \`M\`` | import identifier |
| `E0200` | `duplicate import \`M\`` | second import |
| `E0200` | `circular import:` | closing import |
| `E0200` | `module \`M\` cannot import itself` | import identifier |

### Cross-module names (RFC-0026)

| Code | Message | Span target |
|------|---------|-------------|
| `E0200` | `unknown module \`M\`` | module in qualified name |
| `E0200` | `\`N\` is not exported from module \`M\`` | item identifier |
| `E0200` | `\`N\` is private to module \`M\`` | item identifier |
| `E0200` | `unknown function \`M.N\`` | qualified call |
| `E0200` | `unknown struct type \`M.N\`` | qualified type |

### Visibility (RFC-0027)

| Code | Message | Span target |
|------|---------|-------------|
| `E0100` | `visibility modifier \`pub\` is not allowed here` | `pub` keyword |
| `E0200` | `duplicate \`main\` function in program` | second `main` name |

### Type checking (cross-module, extends v0.3)

| Code | Message | Span target |
|------|---------|-------------|
| `E0200` | `argument type mismatch` | argument expr |
| `E0200` | `return type mismatch` | return expr |
| `E0200` | struct field / literal errors | per RFC-0013 § v0.3 rules |

### Multi-file / IO (RFC-0028)

| Code | Message | Span target |
|------|---------|-------------|
| `E0400` | `failed to read module file` | import or path |
| `E0100` | parse errors in imported file | imported file span |

### LLVM / link (RFC-0029)

| Code | Message | Span target |
|------|---------|-------------|
| `E0300` | `undefined symbol` | call site or driver |
| `E0400` | `failed to link multi-module executable` | driver (1:1) |
| `E0300` | `LLVM verifier failed` | driver (1:1) |

---

## 3. Span Rules (v0.4)

| Construct | Span target |
|-----------|-------------|
| Bad import | import module identifier |
| Circular import | import that closes cycle |
| Qualified name error | rightmost identifier (`N` in `M.N`) |
| Private symbol use | use site (qualified name) |
| Module/file mismatch | `module` name token |
| Parse error in dependency | token in **dependency file** (display path in message) |

When showing cross-file errors, the primary span is on the **use site**; optional note points to **definition** in dependency.

Minimum v0.4: message text includes both module names and file paths.

---

## 4. Negative Test Requirements

Every row below must have at least one test using multi-file fixtures or embedded virtual file maps.

### Module layout

| # | Sketch | Assert |
|---|--------|--------|
| M1 | Two files `module math` | duplicate module |
| M2 | `import ghost` | module not found |
| M3 | `math.x` + `math/main.x` | ambiguous layout |
| M4 | File without `module` | missing module |

### Imports

| # | Sketch | Assert |
|---|--------|--------|
| I1 | `import math` twice | duplicate import |
| I2 | `main → math → main` | circular import |
| I3 | `math` imports itself | self import |

### Visibility and names

| # | Sketch | Assert |
|---|--------|--------|
| V1 | Call private `math.helper` from main | private |
| V2 | Use private struct cross-module | private |
| V3 | `unknown.add()` without import | unknown module |
| V4 | `pub` on local | not allowed |

### Positive integration

| # | Sketch | Assert |
|---|--------|--------|
| P1 | `main` imports `math`, calls `math.add` | exit code |
| P2 | Cross-module `geom.Vec2` local | check + llvm + run |
| P3 | IR contains `@xlang.math.add` | snapshot |

---

## 5. Test Naming Convention

```text
rejects_duplicate_module_name
rejects_circular_import
rejects_private_cross_module_call
rejects_unknown_qualified_module
compiles_two_module_math_add_fixture
lowers_cross_module_call_with_mangled_symbol
```

---

## 6. LSP Diagnostics (informative)

When multi-file analysis is enabled:

- Import not found → diagnostic on import span
- Private use → diagnostic on qualified name

CLI and LSP should share `analyze` / `check` logic where possible.

---

## 7. Open Questions

1. Unique subcodes per v0.4 error (`E0201` …)? **Proposed: defer; substring tests.**
2. JSON diagnostic output for multi-file? **Proposed: defer.**

---

## 8. Relationship to RFC-0013

This document is the authoritative **v0.4 extension** to RFC-0013. At implementation time, merge a summary into RFC-0013 § v0.4 or link here as normative.
