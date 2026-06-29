# RFC-0004: Lexical Grammar

## Status

Draft

## Summary

This RFC defines the lexical grammar of XLang v0.1.

The lexical grammar describes how raw source code is converted into tokens before parsing.

The goal is to keep XLang easy to tokenize, easy to parse, easy to format, and friendly to both humans and AI tooling.

---

## 1. Source Text

XLang source files are UTF-8 encoded text files.

The recommended file extension is:

```text
.x
```

Example:

```text
main.x
```

---

## 2. Whitespace

Whitespace separates tokens but is otherwise ignored, except where newlines are used by the parser in future grammar decisions.

Whitespace characters:

```text
space
tab
carriage return
newline
```

The lexer records line and column information for diagnostics.

---

## 3. Newlines

Newlines are significant for source locations and diagnostics.

Newlines do not terminate statements in v0.1. Executable statements and expression statements are terminated by semicolons.

Supported newline styles:

```text
LF      \n
CRLF    \r\n
```

---

## 4. Comments

Line comments start with `//` and continue until the end of the line.

```xlang
// This is a line comment
i32 x = 10;
```

Block comments start with `/*` and end with `*/`.

```xlang
/*
This is a block comment.
*/
i32 x = 10;
```

Nested block comments are an open question.

Current recommendation for v0.1: **do not support nested block comments**.

Documentation comments are supported lexically.

```xlang
/// Adds two integers.
i32 add(i32 a, i32 b) {
    return a + b;
}
```

---

## 5. Identifiers

Identifiers are used for variables, functions, modules, types, and fields.

Initial rule:

```ebnf
identifier = alphabetic_or_underscore, { alphabetic_or_digit_or_underscore } ;
```

Examples:

```xlang
player
player_hp
_add
main2
```

Invalid examples:

```xlang
2player
player-name
```

Unicode identifiers are an open question.

Decision for v0.1: **ASCII identifiers only**.

---

## 6. Keywords

The following words are reserved and cannot be used as identifiers:

```text
module
import

struct
enum
trait

const

if
else
match

for
while
loop

return
break
continue

defer

async
await

parallel
spawn

gpu

unsafe

pub

impl

where

static

type

sizeof
alignof

move
mut
as
in
```

`fn`, `let`, and `var` are **not** keywords in v0.1 — they are valid identifier spellings. The concrete grammar uses C-style declarations only.

Not all keywords are implemented in the MVP compiler.

Some are reserved early to avoid future breaking changes.

---

## 7. Integer Literals

Decimal integer literals:

```xlang
0
1
42
1000
```

Underscores may be used as visual separators:

```xlang
1_000_000
```

Integer base prefixes:

```text
0b1010       binary
0o755        octal
0xFF         hexadecimal
```

Initial MVP requirement:

```text
decimal integers only
```

The parser stores decimal integer literal magnitude before type checking. The MVP type checker accepts only values that fit the backend `i32` subset; the minimum value `-2147483648` is accepted as unary `-` applied to the literal magnitude `2147483648`.

Type suffixes are an open question.

Possible future syntax:

```xlang
42i32
255u8
```

---

## 8. Floating-Point Literals

Examples:

```xlang
3.14
0.5
10.0
1e9
1.5e-3
```

Initial MVP requirement:

```text
float literals may be lexed but do not need to be type-checked or compiled yet
```

---

## 9. String Literals

String literals use double quotes.

```xlang
"hello"
"hello, XLang"
```

Supported escape sequences:

```text
\n
\r
\t
\\
\"
\0
```

Unicode escape syntax is an open question.

Possible future syntax:

```xlang
"\u{1F600}"
```

---

## 10. Character Literals

Character literals use single quotes.

```xlang
'a'
'\n'
```

The exact meaning of `char` is defined in the type-system RFC.

Current recommendation:

```text
char represents a Unicode scalar value in the language model
```

For the MVP, char literals may be lexed but not compiled.

---

## 11. Operators

Initial operators:

```text
+
-
*
/
%
=
==
!=
<
<=
>
>=
&&
||
!
&
|
^
~
<<
>>
->
=>
.
,
:
::
?
;
```

Not all operators are implemented in the MVP compiler.

---

## 12. Delimiters

Delimiters:

```text
(
)
{
}
[
]
```

---

## 13. Token Kinds

Initial token kinds:

```text
Identifier
Keyword

IntegerLiteral
FloatLiteral
StringLiteral
CharLiteral

Plus
Minus
Star
Slash
Percent

Equal
EqualEqual
Bang
BangEqual

Less
LessEqual
Greater
GreaterEqual

Ampersand
AmpersandAmpersand
Pipe
PipePipe
Caret
Tilde

LeftShift
RightShift

Arrow
FatArrow

Dot
Comma
Colon
ColonColon
Question
Semicolon

LeftParen
RightParen
LeftBrace
RightBrace
LeftBracket
RightBracket

EndOfFile
Error
```

---

## 14. Source Locations

Each token must carry source location metadata.

Recommended structure:

```text
Span {
    file_id
    start_byte
    end_byte
    start_line
    start_column
    end_line
    end_column
}
```

During the earliest bootstrap, a simplified structure may be used internally only if diagnostics remain deterministic and tests cover the emitted locations:

```text
Span {
    line
    column
}
```

The high-assurance compiler target requires file IDs, byte ranges, start/end line and column positions, and stable diagnostic rendering.

The MVP lexer implementation stores full token spans with a bootstrap file ID of `0` until multi-file source management is introduced. The parser preserves spans on expression nodes, local declaration and assignment identifiers, local declaration type names, return/if statements, function names, parameter names, parameter types, return types, and struct fields so semantic and backend diagnostics can point at source-level anchors instead of falling back to `1:1`.

---

## 15. Lexical Errors

The lexer must report invalid tokens with source location.

Examples:

```text
unterminated string literal
unterminated block comment
invalid character literal
unknown character
invalid numeric literal
```

Diagnostic example:

```text
error[E0101]: unterminated string literal
 --> main.x:4:12
```

---

## 16. MVP Lexer Requirements

The first lexer implementation must support:

- keywords
- identifiers
- decimal integer literals
- string literals
- line comments
- semicolons
- basic operators
- delimiters
- source locations
- EOF token

The first lexer may postpone:

- float literals
- char literals
- block comments
- binary/octal/hex literals
- nested comments
- Unicode identifiers
- numeric suffixes
- raw strings

---

## 17. Open Questions

1. Unicode identifiers are postponed; v0.1 uses ASCII identifiers only.
2. Block comments are not nestable in v0.1.
3. Newlines are not parser-visible statement terminators in v0.1.
4. Semicolons are required statement terminators in v0.1, not optional terminators.
5. Numeric suffixes are postponed; MVP integer literals are decimal and unsuffixed.
6. Documentation comments are lexed as comments in v0.1; separate doc-comment tokens are postponed.
7. Raw strings are postponed.
8. Escape sequences are validated and interpreted by the lexer in the MVP.
