# RFC-0002: Syntax Principles

## Status

Draft

## Summary

This RFC defines the initial syntax direction of XLang.

The goal is to create a syntax that is modern, readable, easy to parse, easy to format, and friendly to both human developers and AI tooling.

---

## 1. Syntax Goals

XLang syntax should be:

- explicit
- regular
- easy to parse
- easy to format
- low-noise
- familiar to C-like language users
- safe by default
- friendly to compiler diagnostics

---

## 2. File Structure

A source file may start with a module declaration:

```xlang
module main
```

Imports follow the module declaration:

```xlang
import math
import io
```

Example:

```xlang
module main

import io

i32 main() {
    io.println("Hello, XLang");
    return 0;
}
```

---

## 3. Functions

Function declaration:

```xlang
i32 add(i32 a, i32 b) {
    return a + b;
}
```

Void functions explicitly use `void`.

```xlang
void log(str message) {
    print(message);
}
```

---

## 4. Variables

Mutable local declaration:

```xlang
i32 hp = 100;
```

Assignment:

```xlang
hp = hp - 10;
```

Immutable local declaration:

```xlang
const i32 max_hp = 100;
```

Local declarations are type-first and require an initializer in v0.1.

```xlang
i32 hp = 100;
```

Dangerous implicit conversions are not allowed.

Invalid:

```xlang
i32 x = 3.14;
```

Valid:

```xlang
i32 x = cast<i32>(3.14);
```

---

## 5. Structs

Struct declaration:

```xlang
struct Player {
    i32 hp;
    String name;
}
```

Struct field declarations use semicolons because newlines are not statement or field terminators in v0.1.

Struct construction:

```xlang
Player player = Player {
    hp: 100;
    name: String.from("Ava");
};
```

---

## 6. Enums

Enum declaration:

```xlang
enum Option<T> {
    Some(T)
    None
}
```

Result type:

```xlang
enum Result<T, E> {
    Ok(T)
    Err(E)
}
```

---

## 7. Pattern Matching

Initial direction:

```xlang
match result {
    Ok(value) => {
        return value;
    }
    Err(error) => {
        return 0;
    }
}
```

Pattern matching should be exhaustive for enums.

---

## 8. Control Flow

If expression or statement:

```xlang
if hp <= 0 {
    return;
} else {
    hp = hp - 1;
}
```

Loops:

```xlang
while running {
    update();
}
```

```xlang
loop {
    tick();
}
```

For loops are not fully specified yet.

Possible direction:

```xlang
for item in items {
    process(item);
}
```

---

## 9. Semicolons

XLang v0.1 requires semicolons after executable statements and expression statements.

```xlang
i32 x = 10;
i32 y = 20;
return x + y;
```

Semicolons are not required after structural declarations or block constructs such as `module`, `import`, function declarations, `struct`, `enum`, `if`, `while`, `loop`, or `match`, unless a future grammar allows one of those constructs to appear as an expression statement. Struct field declarations are semicolon-terminated inside the struct body.

---

## 10. Comments

Line comment:

```xlang
// This is a comment
```

Block comment:

```xlang
/*
This is a block comment.
*/
```

Documentation comment:

```xlang
/// Adds two integers.
i32 add(i32 a, i32 b) {
    return a + b;
}
```

---

## 11. Keywords

Initial keyword list:

```text
module
import

fn

struct
enum
trait

const
let
var

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
```

Decision for v0.1: avoid `interface` and use only `trait`. There is no `fn`, `let`, or `var` keyword — functions and locals use C-style syntax (`i32 name(…)`, `i32 x = …;`, `const i32 x = …;`).

---

## 12. Open Questions

This RFC leaves the following decisions open:

1. Semicolons are required after executable statements and expression statements.
2. `void` is the explicit return type for functions that do not return a value.
3. `interface` does not exist separately from `trait` in v0.1.
4. `if` starts as a statement in the MVP; expression form is postponed.
5. `match` starts as a statement in the MVP; expression form is postponed.
6. Local initializer inference is postponed; function signatures and local declarations remain explicit.
7. Generic type syntax uses `Name<T>` and `Name<T, E>` as the intended direction, with full semantics postponed.
8. References use `&T` and `&mut T` as the intended direction; raw pointers need a later RFC.
