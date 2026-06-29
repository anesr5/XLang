# RFC-0028: Multi-File Compilation

## Status

Draft

## Summary

This RFC defines the **compiler driver** behavior for loading, checking, and code-generating **multiple XLang modules** in v0.4.

The CLI still accepts one **entry file**, but compilation may involve many `.x` sources discovered through imports.

---

## 1. Compilation Unit

A **compilation unit** is the closure of:

- the entry module file
- all modules reachable via `import` (transitive)

All modules share one logical program for type checking but emit **separate LLVM modules** (RFC-0029).

```text
CompilationUnit {
    entry: ModuleId,
    modules: HashMap<ModuleName, ModuleSource>,
    import_graph: DiGraph<ModuleName>,
}
```

---

## 2. CLI Behavior

### Commands

`check`, `emit-llvm`, `build`, and `run` accept one primary source path:

```bash
x run examples/v0.4/main.x
x check project/main.x
x build project/main.x
```

The driver:

1. Sets project root = parent directory of `main.x`
2. Loads entry file → module `main`
3. Resolves imports recursively (RFC-0025)
4. Type-checks entire unit
5. Emits IR per module (or single combined strategy — see RFC-0029)
6. Links to executable (`build` / `run`)

### Output paths (v0.4 extension)

**Proposed layout:**

```text
build/
  main.ll
  math.ll
  main.exe
```

Or object files via clang:

```text
build/
  main.ll → main.o
  math.ll → math.o
  linked executable
```

Single-module v0.1–v0.3 behavior remains: `build/main.ll` + `build/main.exe`.

---

## 3. Phases

```text
1. discover(entry_path)
2. for each module in deterministic order:
     lex → parse → collect exports
3. build global ExportRegistry
4. for each module:
     typeck(module, registry)
5. for each module:
     emit_llvm(module) → verify
6. link(all modules) → executable
```

Type checking is **global**: module B may reference exports of module A only if A was loaded and appears in B's import closure.

---

## 4. Determinism

Module processing order for codegen:

1. Sort module names **ascending**
2. Within a module, items in source order

Diagnostic order:

1. Module name ascending
2. Source line ascending

File system enumeration must not rely on OS-dependent directory order without sorting.

---

## 5. Error Handling

Failures in any module abort the whole compilation:

- Parse error in imported file → fail with span in that file
- Type error in `math.x` → fail even if entry is `main.x`
- Missing module → fail at import site in importer

Cross-file spans in diagnostics:

```text
error[E0200]: `helper` is private to module `math`
 --> math.x:4:5
   referenced from main.x:7:12
```

**Stretch for v0.4:** secondary **note** span pointing to reference site. Minimum: message names both modules.

---

## 6. `check` vs `build`

| Command | Loads imports | Typecheck | LLVM | Link |
|---------|:-------------:|:---------:|:----:|:----:|
| `check` | yes | yes | no | no |
| `emit-llvm` | yes | yes | yes (print) | no |
| `build` | yes | yes | yes | yes |
| `run` | yes | yes | yes | yes |

---

## 7. LSP and IDE (informative)

Initial v0.4 may compile single open file only in LSP (degraded). Full multi-file analysis is a stretch goal:

- Load imports from disk relative to open file's directory
- Publish diagnostics for imported files when changed

Document limitation in release notes if not shipped.

---

## 8. Test Layout

Multi-file tests in `compiler/tests/` or inline with virtual filesystem:

```text
fixtures/v0_4/math_add/
  main.x
  math.x
  expected_exit.txt
```

Integration test runs `x run main.x` from fixture directory.

---

## 9. Negative Tests (required)

| Test | Assert |
|------|--------|
| Entry imports broken module | compile fails |
| Type error in dependency | compile fails before link |
| Two-module link success | exit code |

---

## 10. Open Questions

1. Single combined LLVM module vs per-module IR? **Proposed: per-module IR + link (RFC-0029).**
2. Incremental compilation? **Proposed: no in v0.4 — full rebuild.**
