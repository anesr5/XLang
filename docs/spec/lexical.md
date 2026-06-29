# Lexical structure

## Whitespace

These characters are ignored except for separating tokens and tracking source locations:

- space (` `)
- tab (`\t`)
- carriage return (`\r`)
- newline (`\n`)

## Comments

| Form | Status |
|------|--------|
| Line comment `// …` | **Supported** — runs to end of line |
| Block comment `/* … */` | **Supported** - non-nesting |
| Doc comment `/// …` | **Supported lexically** - skipped like a line comment |

```xlang
// This is valid.
/// This is also skipped.
/* Block comments may span
   multiple lines. */
i32 main() { return 0; }
```

## Identifiers

Identifiers name variables, functions, modules, types, and struct fields.

```ebnf
identifier = ( ASCII-letter | '_' ), { ASCII-letter | ASCII-digit | '_' } ;
```

- **ASCII only** — Unicode identifiers are not supported.
- Identifiers cannot be keywords (see below).

Valid examples: `main`, `player_hp`, `_internal`, `x2`

Invalid examples: `2player`, `player-name`

## Keywords

The following words are reserved. Using them as identifiers is a lex/parse error in contexts that expect an identifier.

```text
module  import
struct  enum  trait
const
if  else  match
for  while  loop
return  break  continue
defer
async  await
parallel  spawn
gpu
unsafe
pub  impl  where
static  type
sizeof  alignof
move  mut  as  in
true  false
```

### Keywords used in v0.1 syntax

| Keyword | Role |
|---------|------|
| `module` | Module declaration |
| `import` | Import declaration |
| `struct` | Struct declaration |
| `const` | Immutable local binding |
| `return` | Return statement |
| `if`, `else` | Conditional statement |
| `true`, `false` | Boolean literals |

### Reserved but unused in v0.1 syntax

Including all control-flow, concurrency, and GPU keywords listed above. They tokenize as keywords but are not part of the current grammar.

`fn`, `let`, and `var` are **not** reserved — they are valid identifier spellings.

## Literals

### Integer literals

Decimal integer literals tokenize as signed 64-bit values during lexing. The type checker then restricts **expression** integer literals to the **`i32` range**:

| Form | Rule |
|------|------|
| Positive `N` | `0 ≤ N ≤ 2_147_483_647` (`i32::MAX`) |
| Unary `-N` | Special case: `-2147483648` is allowed as `i32::MIN` |
| Unary `-N` (other) | `N` must be `≤ 2_147_483_648` |

Examples:

```xlang
return 42;
return -1;
return -2147483648;   // i32::MIN
```

Out-of-range literals are rejected at type-check time.

### Floating-point literals

The lexer recognizes tokens such as `3.14`, but **expressions cannot use float literals** today. A float token where an expression is expected produces a parse error.

### Boolean literals

```xlang
true
false
```

### String literals

Double-quoted strings with optional escape sequences:

| Escape | Meaning |
|--------|---------|
| `\n` | newline |
| `\r` | carriage return |
| `\t` | tab |
| `\\` | backslash |
| `\"` | double quote |
| `\0` | null |

Other `\x` escapes are rejected. Unterminated strings (including raw newlines inside quotes) are errors.

String literals type-check as `str` but **cannot be lowered to LLVM** in the current backend.

### Character literals

The lexer recognizes single-quoted character literals (e.g. `'a'`), but **expressions cannot use character literals** today.

## Operators and punctuation (lexed)

The lexer produces tokens for the following. Only the subset listed in [Expressions](expressions.md) is usable in expression grammar.

**Arithmetic / logic:** `+` `-` `*` `/` `%` `!`

**Comparison:** `==` `!=` `<` `<=` `>` `>=`

**Short-circuit logic:** `&&` `||`

**Bitwise (lexed only, not in expressions):** `&` `|` `^` `~` `<<` `>>`

**Other (lexed only, not in current grammar):** `->` `=>` `.` `::` `?` `[` `]`

**Punctuation used in grammar:** `(` `)` `{` `}` `;` `,` `:` `=`

## Token spans

Every token carries a source span (start/end byte offset, line, column). Diagnostics use these spans where available.
