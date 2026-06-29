# RFC-0017: Index Expressions and Bounds Checking

## Status

Draft

## Summary

This RFC defines **index expressions** (`arr[i]`) for reading and assigning array elements, and mandates **runtime bounds checking** in v0.2.

Every index operation must verify `0 <= index < length` before memory access in generated code.

---

## 1. Syntax

Extend RFC-0005 expressions:

```ebnf
primary     = ...
            | index_expr ;

index_expr  = postfix_expr, "[", expr, "]" ;
postfix_expr = identifier_or_call ;   /* extended in future for chained indexing */
```

For v0.2, the postfix base must be:

- an **identifier** referencing an array local, or
- a nested **index expression** is **not** required in v0.2 (single-dimensional only)

Examples:

```xlang
i32 x = arr[i];       // read
arr[i] = x + 1;       // assign (statement form via assignment_stmt to index lvalue)
arr[i + 1] = 0;       // index expression may be arithmetic
```

### Assignment to index

Assignment uses the existing assignment statement with an index expression on the left:

```ebnf
assignment_stmt = indexable_lvalue, "=", expr, ";" ;
indexable_lvalue = identifier, { "[", expr, "]" } ;
```

v0.2: exactly one `[ … ]` pair (single-dimensional).

---

## 2. Typing

### Index type

The index expression must have type **`i32`**.

```text
error[E0200]: array index must be i32, got Bool
```

### Base type

The indexed value must be an **array type** `T[N]`.

Indexing a scalar local, function, or non-array value:

```text
error[E0200]: cannot index value of type I32
```

### Result type (read)

`arr[i]` in expression position has type **`T`** (element type).

### Assignability

In `arr[i] = expr;`, `expr` must match `T` exactly.

Assigning through a `const` array binding is rejected (RFC-0016).

---

## 3. Bounds Checking Semantics

### Requirement

For every index read or write, the compiler must emit a **runtime check**:

```text
0 <= index && index < N
```

where `N` is the compile-time array length associated with the base array value.

If the check fails, the program must **abort execution** in a predictable way (see §5).

### Compile-time constants

When the index is a compile-time-known integer constant outside `[0, N)`, the compiler **may**:

- reject the program at compile time with a definite out-of-bounds error, **or**
- still emit the runtime check

**Decision for v0.2:** reject constant out-of-bounds indices at **compile time**:

```text
error[E0200]: index 5 is out of bounds for array of length 4
```

Constant in-bounds indices may still emit checks or elide them; elision is an optimization, not a semantic requirement.

### Non-constant indices

Always emit runtime checks in v0.2 backend.

---

## 4. Evaluation Order

1. Evaluate index expression to `i32`
2. Evaluate bounds check against `N`
3. On success, compute element address and load or store
4. On failure, execute abort path

For assignment `arr[i] = expr`:

1. Evaluate index
2. Bounds-check
3. Evaluate `expr`
4. Store to element

The index is not re-evaluated on store.

---

## 5. Failure Behavior

On bounds failure:

**Decision for v0.2:** call a private runtime helper or inline **`llvm.trap`** after the failing branch.

Diagnostic at compile time does not occur — this is a **runtime fault**.

Optional future work: `xlang.bounds_fail` with line number metadata.

The process should terminate with non-zero exit status when run under `x run`; exact exit code is platform-defined unless standardized later.

Programs must not continue after a failed bounds check.

---

## 6. LLVM Lowering

Detailed patterns: [RFC-0006 § v0.2](RFC-0006-llvm-ir-lowering-rules.md).

### Address calculation

```llvm
; base: ptr to [N x i32]
; idx: i32
%0 = icmp sge i32 %idx, 0
%1 = icmp slt i32 %idx, N
%ok = and i1 %0, %1
br i1 %ok, label %in_bounds, label %trap

in_bounds:
  %gep = getelementptr [N x i32], ptr %base, i32 0, i32 %idx
  ...
```

Use `getelementptr inbounds` only **after** the guard (or use guarded GEP pattern that never invokes UB on OOB).

### Load

```llvm
%val = load i32, ptr %gep
```

### Store

```llvm
store i32 %val, ptr %gep
```

---

## 7. Interaction with Loops

Typical idiom:

```xlang
i32[4] xs = { 0, 0, 0, 0 };
i32 i = 0;
while i < 4 {
    xs[i] = i * 2;
    i = i + 1;
}
```

If the loop invariant guarantees `0 <= i < 4`, bounds checks still emit unless proven and elided by a future optimization pass. v0.2 **always emits** checks for non-constant indices.

---

## 8. Diagnostics

### Compile time (RFC-0013)

| Code | Condition |
|------|-----------|
| `E0200` | Index not `i32` |
| `E0200` | Base not array |
| `E0200` | Assign through `const` element |
| `E0200` | Constant index out of range |
| `E0100` | Malformed index syntax |

### Runtime

No source diagnostic — trap / abort.

Negative tests cover compile-time cases only; runtime trap may be covered by an integration test that expects non-zero exit when feeding invalid index (future harness).

---

## 9. Negative Tests (required)

| Source | Expected |
|--------|----------|
| `xs[true] = 0;` | Index type error |
| `scalar[0] = 0;` | Cannot index scalar |
| `i32[2] a = {0,0}; a[2] = 1;` | Constant OOB (literal index 2) |
| `const i32[1] a = {0}; a[0] = 1;` | Const array assign |
| `a[-1] = 0;` with literal `-1` | Constant OOB if folded |

---

## 10. Explicit Non-Goals

- Negative indexing (`arr[-1]`)
- Slice syntax
- Unchecked `@unchecked(arr, i)` access
- SIMD gather/scatter
- Pointer offset arithmetic
- Multi-dimensional chained indexing without intermediate RFC

---

## 11. Open Questions

1. Should runtime bounds failures include filename/line in a future runtime hook?
2. When should bounds checks be elided via dataflow / loop analysis?
3. Should `usize` indices be introduced before or after pointer support?
