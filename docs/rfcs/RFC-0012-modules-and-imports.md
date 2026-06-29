# RFC-0012: Modules and Imports

## Status

Draft

## Summary

This RFC defines the syntax-only module and import surface for the v0.1 MVP.

Modules and imports are parsed and preserved in the AST, but they do not yet affect name resolution, file loading, or LLVM symbol generation.

---

## 1. Syntax

```ebnf
program     = [ module_decl ], { import_decl }, { item }, EOF ;
module_decl = "module", identifier ;
import_decl = "import", identifier ;
```

Module and import declarations are not semicolon-terminated.

Example:

```xlang
module main
import math
import io

i32 main() {
    return 0;
}
```

---

## 2. Ordering

If present, `module` must appear before imports and items.

Imports must appear before all items. The MVP parser rejects imports after function or struct declarations.

---

## 3. Semantics

Imports are syntax-only in v0.1.

**Superseded by v0.4:** [RFC-0023](RFC-0023-v0-4-roadmap-and-scope.md) through [RFC-0030](RFC-0030-v0-4-diagnostics.md) define real module identity, import resolution, multi-file compilation, and `pub` visibility.

Duplicate imports are preserved as written and have no semantic effect yet in v0.1–v0.3.

Dotted paths, aliases, visibility, package roots, file discovery, and multi-file compilation are postponed.

---

## 4. Open Questions

1. Should imports use dotted paths such as `core.io`?
2. Should duplicate imports be rejected once imports become semantic?
3. How should module names map to files and directories?
4. Should module declarations be required in multi-file mode?
