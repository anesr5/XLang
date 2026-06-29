# RFC-0005: Concrete Grammar and EBNF

## Status

Draft

## Summary

This RFC defines the concrete grammar for the XLang v0.1 MVP.

The grammar is intentionally small. It matches the current Rust bootstrap compiler: source files may contain an optional module declaration, zero or more imports, top-level struct declarations, and function declarations. Executable statements require semicolons. Struct declarations are parsed, but struct construction, field access, layout, and backend lowering are postponed.

RFC-0004 defines lexical tokens. This RFC starts after lexing.

---

## 1. Grammar Goals

The v0.1 concrete grammar should be:

- deterministic
- easy to parse with recursive descent
- easy to format
- friendly to snapshot testing
- explicit about statement termination
- small enough to lower directly to LLVM for the MVP backend subset

Newlines do not terminate statements in v0.1.

---

## 2. EBNF Conventions

This RFC uses the following notation:

```ebnf
rule        = production ;
optional    = [ production ] ;
repetition  = { production } ;
choice      = production | production ;
terminal    = "literal" ;
```

Identifier, integer, string, and comment tokenization is defined by RFC-0004.

---

## 3. Program Structure

```ebnf
program     = [ module_decl ], { import_decl }, { item }, EOF ;
module_decl = "module", identifier ;
import_decl = "import", identifier ;
item        = struct_decl | function_decl ;
```

Module and import declarations are not semicolon-terminated in v0.1.

The current compiler accepts simple identifier module and import names only. Dotted module paths are postponed.

---

## 4. Struct Declarations

```ebnf
struct_decl = "struct", identifier, "{", { field_decl }, "}" ;
field_decl  = type_name, identifier, ";" ;
```

Example:

```xlang
struct Player {
    i32 hp;
    bool alive;
}
```

Struct field declarations are semicolon-terminated because newlines are not grammar terminators.

MVP implementation note: top-level struct declarations are parsed and retained in the AST. Function signatures may not use named struct types yet, and the LLVM backend does not lower structs.

Semantic validation rejects duplicate struct names and duplicate field names within a single struct. Distinct structs may reuse the same field names.

---

## 5. Function Declarations

```ebnf
function_decl = type_name, identifier, "(", [ param_list ], ")", block ;
param_list    = param, { ",", param } ;
param         = type_name, identifier ;
```

Use `void` as the return type for functions that do not return a value (e.g. `void tick()`).

MVP entry-point rule: a program must define `i32 main()` with no parameters.

Example:

```xlang
i32 add(i32 a, i32 b) {
    return a + b;
}
```

---

## 6. Types

```ebnf
type_name = identifier ;
```

The parser accepts identifier-shaped type names. The current type checker recognizes:

```text
i32
bool
str
void
```

Named types are parsed for future struct support. They are not yet accepted in function signatures.

The LLVM MVP backend lowers only:

```text
i32
bool
void
```

Semantic restriction: `void` is valid as an explicit function return type (e.g. `void tick()`). It is not valid as a parameter type or as the type of a first-class value.

---

## 7. Blocks and Statements

```ebnf
block      = "{", { statement }, "}" ;
statement  = binding_stmt
           | assignment_stmt
           | return_stmt
           | if_stmt
           | expr_stmt ;

binding_stmt    = [ "const" ], type_name, identifier, "=", expr, ";" ;
assignment_stmt = identifier, "=", expr, ";" ;
return_stmt     = "return", [ expr ], ";" ;
expr_stmt       = expr, ";" ;
```

Typed local declarations create mutable bindings by default. A declaration prefixed with `const` creates an immutable binding. Initializers are required for all MVP local declarations.

Within one function scope, parameter names must be unique and local bindings may not redeclare an existing parameter or local binding. Nested lexical scopes are postponed, so bindings declared inside `if` branches reserve their names in the enclosing function binding namespace. Branch-local bindings are not usable after the `if` until a later definite-assignment and lexical-scope model is introduced.

Executable statements and expression statements are semicolon-terminated in v0.1:

```xlang
i32 x = 40;
return x + 2;
```

---

## 8. If Statements

```ebnf
if_stmt = "if", expr, block, [ "else", block ] ;
```

Example:

```xlang
if x == 0 {
    return 1;
} else {
    return x;
}
```

MVP decision: `if` is a statement, not an expression. `else if` syntax is postponed; nested `if` inside an `else` block is available.

---

## 9. Expressions

Expression parsing is precedence-based and left-associative for binary operators.

```ebnf
expr        = logical_or ;
logical_or  = logical_and, { "||", logical_and } ;
logical_and = equality, { "&&", equality } ;
equality    = comparison, { ( "==" | "!=" ), comparison } ;
comparison  = term, { ( "<" | "<=" | ">" | ">=" ), term } ;
term        = factor, { ( "+" | "-" ), factor } ;
factor      = unary, { ( "*" | "/" | "%" ), unary } ;
unary       = ( "-" | "!" ), unary | primary ;
primary     = integer_literal
            | string_literal
            | "true"
            | "false"
            | identifier_or_call
            | "(", expr, ")" ;

identifier_or_call = identifier, [ "(", [ argument_list ], ")" ] ;
argument_list      = expr, { ",", expr } ;
```

The frontend type checker currently supports integer, boolean, and string expressions. The LLVM backend supports integer and boolean expressions, calls, unary operators, binary arithmetic, comparisons, equality, and boolean `&&`/`||`.

---

## 10. Current MVP Acceptance Examples

```xlang
module main

i32 add(i32 a, i32 b) {
    return a + b;
}

i32 main() {
    i32 x = add(40, 2);
    return x;
}
```

```xlang
struct Pair {
    i32 left;
    i32 right;
}

i32 main() {
    i32 x = 1;
    x = x + 41;
    return x;
}
```

---

## 11. Parser Snapshot Tests

Parser tests should snapshot stable AST shapes for representative accepted programs:

- module and imports
- function declarations with zero, one, and multiple parameters
- semicolon-terminated bindings, assignments, returns, and expression statements
- nested expression precedence
- function calls
- `if` with and without `else`
- parsed top-level structs with semicolon-terminated fields

Negative parser tests should cover:

- missing semicolon after executable statements
- missing semicolon after struct fields
- malformed function parameter lists
- malformed return types
- unterminated blocks

Snapshot output should be deterministic and should not depend on pointer addresses, hash-map iteration order, local machine paths, or host target triples.

MVP parser implementation note: expression nodes carry source spans; binding and assignment statements carry identifier spans; binding annotations carry type spans; return and `if` statements carry keyword/condition anchors; function signatures carry name, parameter, and return-type spans; and struct fields carry name/type spans. This is the current diagnostic bridge from token spans to semantic and backend checks. Full block spans and richer type-reference nodes remain a later high-assurance expansion.

---

## 12. Open Questions

1. Should module and import paths grow from single identifiers to dotted paths such as `core.io`?
2. Should trailing commas be accepted in parameter and argument lists?
3. Should `else if` become syntax sugar, or remain a nested `if` inside an `else` block?
4. Should `if` become an expression after the MVP statement form is stable?
5. What is the concrete grammar for struct construction and field access?
6. Should future compile-time constants reuse `const` syntax, or introduce a separate item-level declaration form?
