# Grammar

Concrete syntax for the **implemented** XLang v0.1 subset. Informal EBNF uses braces for repetition; source block comments use `/* ... */`.

## Program

```ebnf
program     = [ module_decl ], { import_decl }, { item }, EOF ;

module_decl = "module", identifier ;
import_decl = "import", identifier ;

item        = struct_decl | function_decl ;
```

Module and import declarations are **not** semicolon-terminated.

Only simple identifier names are accepted (no dotted paths).

## Struct declarations

Parsed and stored in the AST. Not usable in expressions or codegen.

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

Field types use the same `type_name` production as functions. Duplicate field names inside one struct are diagnosed.

## Function declarations

```ebnf
function_decl = type_name, identifier, "(", [ param_list ], ")", block ;

param_list    = param, { ",", param } ;
param         = type_name, identifier ;

block         = "{", { statement }, "}" ;
```

Examples:

```xlang
i32 add(i32 a, i32 b) {
    return a + b;
}

void tick() {
    return;
}

i32 main() {
    return 0;
}
```

- Return type appears **before** the function name (C-style).
- Use `void` for functions that return no value.
- There is no `fn` keyword and no `-> return_type` suffix.

## Type names

```ebnf
type_name = identifier ;
```

Recognized built-in identifiers:

| Spelling | Meaning |
|----------|---------|
| `i32` | 32-bit signed integer |
| `bool` | Boolean |
| `str` | String (frontend only) |
| `void` | No value / void return |

Any other identifier is a **named type** (for future struct support). Named types are rejected in function signatures by the type checker.

## Statements

```ebnf
statement       = binding_stmt
                | assignment_stmt
                | return_stmt
                | if_stmt
                | expr_stmt ;

binding_stmt    = [ "const" ], type_name, identifier, "=", expr, ";" ;
assignment_stmt = identifier, "=", expr, ";" ;
return_stmt     = "return", [ expr ], ";" ;
if_stmt         = "if", expr, block, [ "else", block ] ;
expr_stmt       = expr, ";" ;
```

### Binding statements

| Form | Mutability |
|------|------------|
| `type name = expr;` | **Mutable** — may be reassigned |
| `const type name = expr;` | **Immutable** — assignment is an error |

Type annotation is **required**. Inference from the initializer alone is not supported.

Examples:

```xlang
i32 x = 10;
const i32 limit = 100;
bool flag = true;
```

### If statements

`if` is a **statement**, not an expression. No trailing semicolon is required after the closing `}` of an `if`/`else` chain.

```xlang
if x == 0 {
    return 1;
} else {
    return x;
}
```

`else if` is not syntactic sugar; nest an `if` inside `else` instead.

## Expressions

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
            | "true" | "false"
            | identifier_or_call
            | "(", expr, ")" ;

identifier_or_call = identifier, [ "(", [ argument_list ], ")" ] ;
argument_list      = expr, { ",", expr } ;
```

All binary operators are **left-associative** except short-circuit `&&` and `||`, which are evaluated with conditional control flow in the backend.

Parentheses override precedence.

## Statement termination

Every executable statement ends with `;`. This includes:

- local declarations
- assignments
- `return`
- expression statements
- struct field declarations

Block delimiters `{` `}` do **not** replace semicolons.
