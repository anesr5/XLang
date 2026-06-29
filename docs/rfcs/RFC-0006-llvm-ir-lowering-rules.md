# RFC-0006: LLVM IR Lowering Rules

## Status

Draft

## Summary

This RFC defines the LLVM IR lowering contract for the XLang v0.1 MVP compiler.

The compiler is implemented in Rust and lowers directly to LLVM IR through Inkwell `0.9` with the LLVM `22.1` feature. XLang does not use generated C as an intermediate representation. Generated modules must pass `Module::verify()` before textual IR is printed, written to disk, linked, or executed.

The current native build path writes verified LLVM IR and invokes `clang` for native linking.

---

## 1. Backend Goals

The v0.1 LLVM backend should be:

- direct: AST or checked AST to LLVM IR through Inkwell
- deterministic for the same source and target triple
- verifier-gated with `Module::verify()`
- small enough to audit
- explicit about unsupported language features
- covered by LLVM IR snapshot tests
- linkable with native LLVM tooling

The backend must not generate C as an intermediate representation.

---

## 2. Supported MVP Surface

The current backend supports:

- `i32`
- `bool`
- `void`
- function declarations
- function calls
- local bindings
- assignments
- returns
- integer literals
- boolean literals
- variable references
- unary `-` for `i32`
- unary `!` for `bool`
- `+`, `-`, `*`, `/`, `%`
- `==`, `!=`, `<`, `<=`, `>`, `>=`
- `&&`, `||`
- `if` statements with optional `else`

Struct declarations are parsed only. Struct construction, field access, layout, ABI rules, and LLVM lowering are postponed.

Strings may pass frontend checking in limited cases, but the LLVM MVP backend rejects string lowering.

---

## 3. Module Lowering

Each source program lowers to one LLVM module.

Current module name:

```text
xlang
```

The lowering order is:

1. Reject backend-unsupported types and expressions.
2. Declare all functions.
3. Emit function bodies.
4. Verify the LLVM module with `Module::verify()`.
5. Return the verified textual LLVM IR.

`emit-llvm` prints only verified LLVM IR.

`build` writes verified LLVM IR to `build/main.ll` and invokes `clang` to produce the native executable.

---

## 4. Target Triple

The target triple is part of the backend contract because textual LLVM IR and native linking can vary by target.

Decision: the compiler exposes a configurable target triple for backend output and tests.

Implemented CLI shape:

```bash
x emit-llvm --target x86_64-pc-windows-msvc examples/main.x
x build --target x86_64-pc-windows-msvc examples/main.x
x emit-llvm examples/main.x --target x86_64-pc-windows-msvc
```

The option may appear before or after the file path. The environment variable `XLANG_TARGET_TRIPLE` provides the same setting for command invocations that do not pass `--target`.

Implemented library direction:

```text
CompileOptions {
    target_triple: Option<String>
}
```

Current bootstrap behavior: the backend uses the configured target triple when provided. Otherwise, it sets a known host-derived triple when the compiler target matches one of the supported host triples.

Snapshot tests should pin the target triple instead of relying on host defaults.

---

## 5. Type Lowering

```text
XLang i32  -> LLVM i32
XLang bool -> LLVM i1
XLang void -> LLVM void
```

`void` is valid as a function return type. It is not valid as a parameter type, local binding value, returned expression value, or call argument value. Void-returning calls are allowed only where no value is consumed, such as expression statements.

Unsupported in the LLVM MVP:

- `str`
- named struct types
- arrays
- references
- raw pointers
- integer widths other than `i32`
- unsigned integers
- floating-point types
- generics

Unsupported types must produce diagnostics before invalid LLVM IR is emitted.

Source-level semantic diagnostics should use parser-preserved spans where available. The current MVP preserves expression spans, binding/assignment identifier spans, statement anchors for return/if checks, binding annotation spans, and function-signature spans. Backend unsupported-type and unsupported-expression diagnostics use those source anchors where available. Backend-internal LLVM failures may still use synthetic compiler locations until a richer diagnostic model is introduced.

---

## 6. Function Lowering

XLang functions lower to LLVM functions with matching names.

Example XLang:

```xlang
fn add(a: i32, b: i32) -> i32 {
    return a + b;
}
```

Expected LLVM shape:

```llvm
define i32 @add(i32 %a, i32 %b) {
entry:
  %addtmp = add i32 %a, %b
  ret i32 %addtmp
}
```

Parameter names should be preserved where practical to keep IR readable and snapshots reviewable.

Void functions that reach the end of the body without an explicit return emit `ret void`.

Non-void functions are rejected by the frontend if they may exit without returning a value.

MVP entry point: `main` must have no parameters and must return `i32`.

---

## 7. Local Values and Assignments

The current MVP lowers function parameters and local bindings to stack slots with `alloca`, `store`, and `load`.

This conservative model is intentionally easy to audit and gives assignments stable semantics across structured control flow:

```text
let x = value;  -> alloca slot for x, then store value
x = value;      -> store value into x's existing slot
use x           -> load from x's slot
```

Parameter names must be unique. Local bindings may not redeclare an existing parameter or local binding in the same function scope.

This is not the final optimized local-lowering model, but it is correct for mutable locals that cross `if` joins. Future optimization may produce `mem2reg`-friendly IR or explicit SSA phi construction after the MVP semantics are stable.

The frontend rejects duplicate binding declarations across `if` branches because the MVP has one function-level binding namespace and no nested lexical-scope model yet. Assignments to predeclared mutable locals are allowed inside branches.

---

## 8. Expression Lowering

Integer literals lower to signed `i32` constants.

Positive integer literal magnitudes must fit in `i32::MAX`. The unary form `-2147483648` is accepted as the MVP spelling for `i32::MIN`; values below that range are rejected before LLVM lowering.

Boolean literals lower to `i1` constants.

Arithmetic operators:

```text
+  -> build_int_add
-  -> build_int_sub
*  -> build_int_mul
/  -> build_int_signed_div
%  -> build_int_signed_rem
```

Comparison operators:

```text
== -> IntPredicate::EQ
!= -> IntPredicate::NE
<  -> IntPredicate::SLT
<= -> IntPredicate::SLE
>  -> IntPredicate::SGT
>= -> IntPredicate::SGE
```

Boolean operators:

```text
&& -> short-circuiting control flow with an i1 phi
|| -> short-circuiting control flow with an i1 phi
!  -> boolean not over i1
```

Decision: `&&` and `||` short-circuit in the MVP. The RHS is lowered into a separate basic block and is evaluated only when needed by the operator semantics.

---

## 9. Call Lowering

Function calls lower to direct LLVM calls.

Example:

```xlang
let x = add(40, 2);
```

Expected LLVM shape:

```llvm
%calltmp = call i32 @add(i32 40, i32 2)
```

Calling unknown functions is rejected before backend emission.

Passing `void` as an argument is rejected.

Calls returning `void` may be used as expression statements, but they do not produce a first-class value.

---

## 10. If Lowering

`if` statements lower to basic blocks:

```text
if.then
if.else
if.end
```

The condition must be `bool` and lower to `i1`.

If a branch does not terminate, it branches to `if.end`. If both branches terminate, the enclosing function state is marked terminated and no join block is used for subsequent instructions.

`if` is not an expression in the MVP, so no phi node is required for an `if` result value.

---

## 11. Module Verification

Every generated module must pass:

```text
Module::verify()
```

Verification failure is a compiler error and must prevent:

- printing IR from `emit-llvm`
- writing IR for `build`
- invoking native linking
- executing the program through `run`

Verifier failures should include the LLVM verifier message in the diagnostic.

---

## 12. Native Linking

The MVP `build` command writes verified LLVM IR to:

```text
build/main.ll
```

It then invokes `clang` when available:

```bash
clang -Wno-override-module build/main.ll -o build/main.exe
```

On non-Windows hosts, the executable name is `build/main`.

When `--target` or `XLANG_TARGET_TRIPLE` is configured, `build` passes the same triple to clang with `-target`. Cross-linking still depends on the host LLVM and platform SDKs being installed.

---

## 13. LLVM IR Snapshot Tests

LLVM IR snapshot tests should cover:

- minimal `fn main() -> i32`
- function declarations and direct calls
- local bindings and assignments
- arithmetic expressions
- comparison expressions
- boolean expressions
- unary expressions
- returns with and without values
- `if` with an `else`
- `if` where both branches return
- backend rejection of unsupported `str` lowering
- parsed structs being ignored by backend lowering until layout support exists

Snapshot tests should normalize or pin:

- target triple
- module name
- temporary value names
- path-dependent diagnostics
- platform-specific executable names where snapshots include build output

Snapshot tests should assert that no C source file, generated C text, or `gcc` invocation is part of the backend path.

---

## 14. Open Questions

1. Should target configuration also include CPU, features, relocation model, code model, and optimization level?
2. Should mutable locals lower through stack slots first, then rely on LLVM optimization, or should the frontend produce SSA joins directly?
3. What is the first supported struct layout and ABI rule set?
4. Should `emit-llvm` support writing to an explicit output path in addition to stdout?
5. Should the compiler eventually invoke LLVM object emission directly instead of linking textual IR through clang?
