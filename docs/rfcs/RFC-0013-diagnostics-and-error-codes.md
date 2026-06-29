# RFC-0013: Diagnostics and Error Codes

## Status

Draft

## Summary

This RFC defines the MVP diagnostic contract.

Diagnostics must carry:

- a stable error-code family
- a human-readable message
- a source span

Rendered diagnostics use:

```text
error[E0200]: return type mismatch: expected I32, got Bool
 --> main.x:3:12
```

---

## 1. Code Families

The MVP reserves these families:

```text
E0001 lexical errors
E0100 parse errors
E0200 type errors
E0300 backend and LLVM lowering errors
E0400 filesystem and process I/O errors
E9999 internal compiler errors
```

The current compiler stores the family as structured diagnostic data and renders the family code with the diagnostic message.

---

## 2. Span Rules

Diagnostics should point to the most specific source span available.

Examples:

- expression type errors point at the expression
- unknown variables point at the variable identifier
- duplicate declarations point at the duplicate name
- function signature errors point at the offending parameter or return type
- unsupported backend types point at the source-level type or expression that caused lowering to fail

When no source span exists, the compiler may use `1:1` as a temporary synthetic location.

---

## 3. Message Rules

Messages should be deterministic and concise.

Messages should name the invalid construct and, when useful, include expected and actual types.

Diagnostics must not depend on hash-map iteration order, absolute local paths, pointer addresses, or host-specific LLVM wording except where wrapping an unavoidable LLVM verifier message.

---

## 4. Open Questions

1. Should individual errors receive unique codes beyond family-level codes?
2. Should diagnostics include notes, help text, and related spans?
3. Should machine-readable JSON diagnostics be emitted by the CLI?

---

## v0.2 Additions (Draft — RFC-0014 through RFC-0017)

This section defines new diagnostics and **required negative tests** for v0.2 features.

### v0.2 Error Code Extensions

Family codes remain unchanged. v0.2 adds these **message-level** conventions within existing families:

| Code | Category | Example message |
|------|----------|-----------------|
| `E0100` | Parse | `expected ';' after break statement` |
| `E0200` | Type | `while condition must be bool, got I32` |
| `E0200` | Type | `break outside of loop` |
| `E0200` | Type | `continue outside of loop` |
| `E0200` | Type | `array length must be at least 1` |
| `E0200` | Type | `array literal length mismatch: expected 4 elements, got 3` |
| `E0200` | Type | `array element type mismatch: expected I32, got Bool` |
| `E0200` | Type | `cannot assign through const array binding` |
| `E0200` | Type | `array index must be i32, got Bool` |
| `E0200` | Type | `cannot index value of type I32` |
| `E0200` | Type | `index 5 is out of bounds for array of length 4` |
| `E0200` | Type | `array type not supported in function signatures yet` |
| `E0300` | Backend | `LLVM backend does not support array element type str` |

Unique subcodes (e.g. `E0201`) may be introduced later; v0.2 tests should assert **family code + message substring + span line**.

### v0.2 Span Rules

| Construct | Span target |
|-----------|-------------|
| Bad while condition | condition expression |
| `break` / `continue` outside loop | keyword token |
| Array length / literal mismatch | array literal or length literal |
| Bad index type | index expression |
| Index on non-array | base identifier |
| Constant OOB index | index expression |
| Const array element assign | index or assignment target |

### v0.2 Negative Test Requirements

Every row below must have at least one test in `compiler/src/lib.rs` (or a dedicated `tests/` module) using `compile::check_source` or equivalent.

#### Loops (RFC-0015)

| # | Source sketch | Assert |
|---|---------------|--------|
| L1 | `while 1 { }` | `bool` condition message |
| L2 | `{ break; }` in `main` | `break outside of loop` |
| L3 | `{ continue; }` in `main` | `continue outside of loop` |
| L4 | `if true { break; }` without while | `break outside of loop` |

#### Arrays (RFC-0016)

| # | Source sketch | Assert |
|---|---------------|--------|
| A1 | `i32[0] a = { };` | length error |
| A2 | `i32[2] a = { 1 };` | literal mismatch |
| A3 | `i32[2] a = { true, 1 };` | element type mismatch |
| A4 | `const i32[1] a = { 0 }; a[0] = 1;` | const assign |

#### Indexing (RFC-0017)

| # | Source sketch | Assert |
|---|---------------|--------|
| I1 | `i32[2] a = {0,0}; i32 x = a[true];` | index type |
| I2 | `i32 x = 0; x[0] = 1;` | cannot index scalar |
| I3 | `i32[2] a = {0,0}; a[2] = 1;` | constant OOB |

#### Backend (RFC-0006 § v0.2)

| # | Source sketch | Assert |
|---|---------------|--------|
| B1 | `str[2] a = { "a", "b" };` in codegen path | `E0300` or backend rejection |

Positive tests should additionally verify:

- `while` IR contains `while.cond` / `while.body` / `while.end` labels
- index lowering contains compare + conditional branch before GEP
- verified module for combined loop + array example

### v0.2 Test Naming Convention

```text
rejects_while_non_bool_condition
rejects_break_outside_loop
rejects_array_literal_length_mismatch
rejects_index_out_of_bounds_constant
lowers_while_with_array_index_and_bounds_check   // positive snapshot
```

### v0.2 Open Questions

1. Should runtime bounds failures ever surface as catchable errors instead of trap?
2. Should negative tests pin full diagnostic strings or only substrings?
3. Should LSP publish the same codes as CLI for v0.2 syntax errors?

---

## v0.3 Additions (Draft — RFC-0018 through RFC-0022)

This section defines new diagnostics and **required negative tests** for v0.3 struct features.

### v0.3 Error Code Extensions

Family codes remain unchanged. v0.3 adds these **message-level** conventions:

| Code | Category | Example message |
|------|----------|-----------------|
| `E0200` | Type | `struct \`Empty\` must have at least one field` |
| `E0200` | Type | `unknown type \`Foo\` in struct \`Player\`` |
| `E0200` | Type | `field type \`str\` is not supported in struct fields yet` |
| `E0200` | Type | `nested struct field type \`Inner\` is not supported yet` |
| `E0200` | Type | `struct type \`Player\` is not supported in function signatures yet` |
| `E0200` | Type | `struct type \`Player\` is not supported as array element type yet` |
| `E0200` | Type | `unknown struct type \`Vec3\`` |
| `E0200` | Type | `struct local \`Vec2\` requires a struct literal initializer` |
| `E0200` | Type | `struct literal length mismatch: expected 2 fields, got 3` |
| `E0200` | Type | `struct field 1 type mismatch: expected I32, got Bool` |
| `E0200` | Type | `cannot access field on value of type I32` |
| `E0200` | Type | `struct \`Vec2\` has no field \`z\`` |
| `E0200` | Type | `cannot assign to field of const binding \`p\`` |
| `E0200` | Type | `field assignment type mismatch: expected I32, got Bool` |
| `E0300` | Backend | `LLVM backend does not support struct field type str` |
| `E0300` | Backend | `LLVM backend does not support nested struct fields yet` |

### v0.3 Span Rules

| Construct | Span target |
|-----------|-------------|
| Unknown struct type in local | type name or annotation |
| Struct literal length/type mismatch | literal or offending element |
| Field on non-struct | field identifier or base name |
| Unknown field name | field identifier |
| Const field assign | field identifier or binding name |
| Field assign type mismatch | value expression |
| Unsupported field type in struct decl | field type span |

### v0.3 Negative Test Requirements

#### Layout (RFC-0019)

| # | Source sketch | Assert |
|---|---------------|--------|
| S1 | `struct Empty { }` | empty struct error |
| S2 | `struct S { Foo x; }` without `Foo` declared | unknown type |
| S3 | `struct S { str name; }` used in codegen path | unsupported field type |

#### Literals (RFC-0020)

| # | Source sketch | Assert |
|---|---------------|--------|
| L1 | `Vec2 p = { 1 };` (2 fields) | literal length mismatch |
| L2 | `Vec2 p = { true, 1 };` | field type mismatch |
| L3 | `Vec2 p;` without initializer | requires initializer |

#### Field access (RFC-0021)

| # | Source sketch | Assert |
|---|---------------|--------|
| F1 | `i32 x = 0; x.y;` | cannot access field on scalar |
| F2 | `p.z` on `Vec2` | no field `z` |
| F3 | `const Vec2 p = {0,0}; p.x = 1;` | const field assign |

#### Backend (RFC-0022)

| # | Source sketch | Assert |
|---|---------------|--------|
| B1 | struct with `str` field reaches codegen | `E0300` |

Positive tests should additionally verify:

- struct type `%Name = type { … }` in IR
- field GEP indices match declaration order
- verified module for struct literal + field read + field assign

### v0.3 Test Naming Convention

```text
rejects_empty_struct
rejects_struct_literal_length_mismatch
rejects_field_access_on_scalar
rejects_const_struct_field_assign
lowers_struct_literal_and_field_access   // positive snapshot
```

### v0.3 Open Questions

1. Should empty structs be allowed for C interop placeholders?
2. Should field typo suggestions (`did you mean 'x'?`) be v0.3 or LSP-only?
