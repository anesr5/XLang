# RFC-0029: LLVM Linking and Symbol Names

## Status

Draft

## Summary

This RFC defines how XLang v0.4 lowers **multiple modules** to LLVM IR and **links** them into one executable without symbol collisions.

Each source module becomes at least one LLVM module. Cross-module calls use **mangled symbol names** that encode the XLang module and item name.

---

## 1. Goals

- Avoid duplicate LLVM symbol errors when two modules define `add`
- Preserve `Module::verify()` per emitted IR unit
- Link with existing **clang** toolchain (no new linker required)
- Keep mangling scheme simple, deterministic, and debuggable

---

## 2. Per-Module LLVM Output

For module `math`:

- LLVM module identifier: `xlang.math` (informative)
- Output file: `build/math.ll` (when multi-module)

For module `main`:

- Contains `main` entry function (unmangled C ABI name — see §4)

Each LLVM module is verified independently before link.

---

## 3. Mangling Scheme (v0.4)

### Functions

Cross-module visible functions (all `pub` functions, and private functions only called internally) lower to:

```text
@xlang.<module>.<function>
```

Examples:

```llvm
define i32 @xlang.math.add(i32 %a, i32 %b) { ... }
define internal i32 @xlang.math.helper() { ... }
```

**Private functions** not referenced across modules may use `internal` linkage or module-local mangling:

```llvm
define internal i32 @xlang.math.helper() { ... }
```

### Struct types

Named LLVM struct types include module prefix to avoid layout collisions:

```llvm
%geom.Vec2 = type { i32, i32 }
%math.Vec2 = type { i32, i32 }   ; distinct type even if identical fields
```

XLang type `geom.Vec2` always lowers to `%geom.Vec2`.

---

## 4. Entry Point `main`

The program entry remains:

```llvm
define i32 @main() { ... }
```

**Decision for v0.4:** only the **entry module** emits `@main`. Other modules must not emit `@main`.

This matches C linker conventions and existing v0.1 `run` pipeline.

---

## 5. Cross-Module Calls

Source:

```xlang
math.add(40, 2)
```

Lowered call:

```llvm
%r = call i32 @xlang.math.add(i32 40, i32 2)
```

The emitter for module `main` declares external prototypes for referenced pub functions:

```llvm
declare i32 @xlang.math.add(i32, i32)
```

---

## 6. Struct Values Across Modules

v0.4 allows **pub struct types** in cross-module function signatures and locals (RFC-0026).

Lowering rules:

- Struct types are module-qualified in IR
- Pass/return of structs uses **by-value** LLVM struct types in the C ABI sense where supported, or pointer-to-stack — **implementation choice** documented in release notes

**Proposed for v0.4 bootstrap:** structs in cross-module **function parameters** deferred if ABI unclear; **locals with imported struct types** and **cross-module calls returning scalars** ship first.

**Minimum v0.4 ship bar (align with RFC-0023):**

- Cross-module calls returning scalars: **required**
- Cross-module struct **type** in locals with literals: **required** if v0.3 stable
- Cross-module struct **by-value ABI** in params/returns: **stretch** — may defer with RFC note

Update RFC-0023 success criteria if deferring struct params — user asked for cross-module struct type usage which includes locals and types.

---

## 7. Link Step

After emitting `build/*.ll`:

```bash
clang -o build/main.exe build/main.ll build/math.ll ...
```

Or compile to `.o` first for clearer errors:

```bash
clang -c build/math.ll -o build/math.o
clang -c build/main.ll -o build/main.o
clang -o build/main.exe build/main.o build/math.o
```

Existing `compile.rs` driver extended; no C-as-IR.

---

## 8. Single-Module Compatibility

When only one module compiles:

- Functions may retain **unmangled** names (`@add`) **or** use mangling consistently

**Proposed:** always mangle with module prefix even for single-file (`@xlang.main.add`) **or** keep legacy unmangled for zero-import single file.

**Recommended:** mangling always on in v0.4 for uniformity; update IR snapshot tests.

Exception: `@main` stays unmangled.

---

## 9. Verification and Tests

- Each `.ll` passes `llvm-as` / `Module::verify()`
- Link step succeeds for two-module fixture
- IR snapshot includes `@xlang.math.add` declaration in main module

---

## 10. Diagnostics

Backend / IO failures:

```text
error[E0400]: failed to link multi-module executable
error[E0300]: undefined symbol `@xlang.math.add` in module `main`
```

See [RFC-0030](RFC-0030-v0-4-diagnostics.md).

---

## 11. Relationship to RFC-0006

RFC-0006 § v0.4 (to be added at implementation time) cross-references this document for linking policy.

---

## 12. Open Questions

1. Itanium-style mangling vs simple dot names? **Proposed: simple `xlang.module.item`.**
2. Cross-module struct ABI in params? **Proposed: defer params/returns to v0.4.1 if needed.**
