# RFC-0001: Vision, Philosophy, and Non-Goals

## Status

Draft

## Summary

This RFC defines the high-level vision of XLang, the core design philosophy, and the non-goals that should guide future decisions.

XLang is a modern systems programming language designed to offer native performance, memory safety without a garbage collector, simple syntax, safe concurrency, and progressive GPU integration.

The language is also designed to be AI-tooling-friendly: easy to parse, analyze, format, refactor, and generate.

---

## 1. Vision

XLang is a next-generation systems programming language for building high-performance software such as:

- native applications
- AI runtimes
- distributed inference engines
- GPU kernels
- game engines
- embedded systems
- networking software
- numerical computing libraries
- operating-system-level components
- cross-platform native tooling

XLang should provide performance close to C and C++, while avoiding common sources of memory unsafety and undefined behavior.

The language should be suitable for both low-level systems programming and high-level performance-oriented application development.

---

## 2. Core Goals

XLang aims to provide:

1. Native performance
2. Memory safety without a garbage collector
3. Explicit error handling
4. No hidden exceptions
5. No dangerous implicit conversions
6. No undefined behavior in safe code
7. Fast compilation
8. Cross-platform compilation
9. Safe concurrency
10. Progressive GPU support
11. Excellent tooling
12. AI-friendly syntax and structure
13. Deterministic, auditable compiler behavior
14. Reproducible compiler outputs for the same source and target

---

## 3. Design Priorities

Language design decisions must follow this priority order:

1. Safety
2. Performance
3. Simplicity
4. Readability
5. Compatibility

When a trade-off is required, XLang should prefer rejecting unsafe code over allowing dangerous behavior.

Performance is critical, but not at the cost of silent unsafety.

The compiler implementation must be held to a high-assurance quality bar: phase outputs must be deterministic, diagnostics must be structured and testable, backend lowering must be auditable, and release builds must pass formatting, linting, unit tests, negative tests, code generation tests, and backend verification gates.

---

## 4. Safety Philosophy

XLang safe code must not allow:

- use-after-free
- double-free
- data races
- null dereference
- uninitialized memory access
- out-of-bounds access
- unsafe implicit numeric conversions
- hidden control flow through exceptions
- undefined behavior

Unsafe operations must be explicit and isolated inside `unsafe` blocks.

Example:

```xlang
unsafe {
    RawPtr ptr = raw_ptr(buffer);
}
```

The `unsafe` keyword does not disable all safety checks. It only allows specific low-level operations that are normally forbidden in safe code.

---

## 5. Performance Philosophy

XLang should expose enough information to the compiler to enable strong optimization.

However, XLang should not hide critical costs from the developer.

The language should make the following costs visible:

- heap allocation
- copying large values
- synchronization
- thread spawning
- GPU transfer
- dynamic dispatch
- unsafe pointer operations

XLang should optimize aggressively, but not magically.

---

## 6. Memory Management Philosophy

XLang does not use a garbage collector.

Memory safety should be achieved through:

- ownership
- move semantics
- borrowing
- lifetimes or lifetime inference
- explicit allocation APIs
- deterministic destruction
- optional `defer`

The exact ownership model will be defined in a dedicated RFC.

Initial direction:

```xlang
String a = String.from("hello");
String b = move a;

Ref<String> r = &b;
MutRef<String> m = &mut b;
```

---

## 7. Error Handling Philosophy

Errors are values.

XLang should not use hidden exceptions for normal error handling.

Example direction:

```xlang
Result<String, IoError> read_file(str path) {
    ...
}
```

The standard library should include:

```xlang
enum Option<T> {
    Some(T)
    None
}

enum Result<T, E> {
    Ok(T)
    Err(E)
}
```

A dedicated RFC will define the error propagation operator and error conventions.

---

## 8. GPU Philosophy

GPU support is a long-term core feature of XLang, but it should be introduced progressively.

The language should eventually support GPU functions such as:

```xlang
gpu void blur(GpuBuffer<f32> input, GpuBuffer<f32> output) {
    ...
}
```

However, `gpu fn` must have strict rules.

Early GPU restrictions may include:

- no recursion
- no arbitrary heap allocation
- no blocking I/O
- restricted pointer operations
- explicit memory buffers
- explicit host-device transfer
- limited standard library support

The compiler may support multiple GPU backends over time:

- CUDA
- SPIR-V / Vulkan Compute
- Metal
- CPU fallback

The first implementation should target only one backend.

---

## 9. AI-Tooling-Friendly Design

XLang should be designed so that both humans and AI tools can understand and manipulate code reliably.

This implies:

- simple grammar
- minimal ambiguous syntax
- no textual macros
- no preprocessor
- stable AST format
- official formatter
- official LSP
- structured compiler diagnostics
- machine-readable compiler output
- clear module system
- predictable naming and layout conventions

Potential tooling commands:

```bash
x format
x check
x test
x ast main.x --format json
x explain E0421
x fix
```

---

## 10. Non-Goals

XLang does not aim to be:

- a scripting language
- a Python replacement
- a JavaScript replacement
- a fully object-oriented language
- a language with classical inheritance
- a language with hidden exceptions
- a language with a garbage collector
- a language with a textual preprocessor
- a language with unrestricted operator overloading
- a language that hides performance costs
- a language that tries to replace every language at once

---

## 11. Initial Target Use Cases

The first versions of XLang should focus on:

- CLI tools
- native libraries
- data-processing tools
- small runtimes
- numerical kernels
- C interoperability
- CPU-first systems programming

GPU support, async runtimes, package management, and advanced concurrency should come after the CPU language core is stable.

---

## 12. Design Constraint

The first successful version of XLang should be small.

The goal is not to design the perfect language immediately.

The goal is to design a minimal, coherent, compilable language that can grow without contradiction.
