# Pillar language reference — 2.0 alpha

The formal logical grammar is in [GRAMMAR.ebnf](GRAMMAR.ebnf).

All snippets here are horizontal views. Import them with `plrc transpose --from
horizontal`; compiler inputs and imports are vertical UTF-8 `.plr` files.

## Source geometry

Read each column top to bottom, then columns left to right. One cell is one Unicode
scalar value, not a byte, UTF-16 code unit or grapheme. Missing cells are spaces;
column boundaries are logical newlines. LF and CRLF are supported. Tabs are errors;
write `\t` inside strings instead. Wide or combining characters can occupy a visual
width different from one cell; source coordinates continue to count scalars.
Identifiers use `[A-Za-z_][A-Za-z_0-9]*` and are case sensitive.

`//` comments end at the logical column boundary. `/* ... */` comments may span
columns, do not nest, and must close. Whitespace and comment contents are retained
for formatting. Sources are limited to 1 MiB, the layout to four million cells,
expression/block/type nesting to bounded depths, and imports to 128 modules. The
limits return diagnostics before generating native code.

## Types and declarations

| Type | Meaning |
| --- | --- |
| `int` | Signed 64-bit integer, checked arithmetic |
| `float` | IEEE-754 binary64 |
| `bool` | `true` or `false`; no numeric truthiness |
| `str` | Immutable, owned UTF-8 string |
| `unit` | No value; not a variable, field or array-element type |
| `[T]` | Array of values of one non-unit type |
| A struct name | Value with declared, named fields |

```text
LT counter: int = 0~
LT inferred = 42~
LT values: [float] = []~
```

`LT` is mutable and requires initialization. An annotation is checked, or a local
type is inferred from the initializer. Empty arrays require a contextual type.
Names cannot be repeated in one scope. Nested blocks may shadow names; the outer
binding is restored afterwards. Parameters share the function body's outer scope.
A shadowing local value cannot be called as a function.

There are no implicit numeric conversions. `int(x)`, `float(x)` and `str(x)` are
explicit conversions. Float-to-int truncates toward zero and checks finite/range
constraints. String numeric conversions reject malformed and out-of-range input;
leading/trailing whitespace is permitted. Float input and literals must be finite.
Decimal integer and float literals and float exponents are supported. The minimum
integer literal `-9223372036854775808` is valid.

## Functions and control flow

```text
FN twice(x: float): float { RET x * 2.0~ }
FN greet(name: str) { println(name)~ RET~ }
FN main(): int { greet("Pillar")~ RET 0~ }
```

Parameters require types. An omitted result type means `unit`. Result-bearing
functions return the declared type on every statically reachable path. Function
signatures are collected first; forward calls and mutual recursion work. Functions
are top-level; function values, nested functions and overloading are not supported.
A module may be checked without a main function. Executables require exactly one
root `FN main(): int`, without parameters. A C ABI adapter returns its exit status;
portable main statuses are 0–255, with out-of-range values diagnosed at runtime.

`IF condition { ... } EL { ... }` requires a bool and a complete `EL`/block pair.
`WH condition { ... }` loops while a bool is true. `break~` and `continue~` affect the
nearest enclosing loop; `BRK~` and `CONT~` are compact aliases. `RET~` is valid for
unit functions. Statically unreachable statements are errors. A literal-true loop
without a break in that loop has no fallthrough; more general conditions are
analyzed conservatively. Execution has a 256-function call-depth limit.

## Expressions

From strongest to weakest: indexing/field access, unary `-`/`!`, `* / %`, `+ -`,
`< <= > >=`, `== !=`, `&&`, `||`. Binary arithmetic associates left. Comparison
chains need explicit parentheses. Operands have matching types. Bool only supports
logical operators and equality; `%` requires int. Str supports `+`, `==`, `!=`.
Logical operators short-circuit. Arguments and elements evaluate left to right;
assignment indices evaluate once, before its right-hand side.

All integer arithmetic is checked in both debug and optimized programs. Zero
divisors and `MIN / -1` or `MIN % -1` fail. Floating-point division also rejects a
zero divisor, including `-0.0`. Other floating-point arithmetic follows IEEE
semantics, including special results; it does not acquire integer overflow rules.

Strings cannot cross a logical column boundary. Escapes are `\n`, `\r`, `\t`,
`\0`, `\"`, `\\`, and `\u{hex}` for valid Unicode scalars. Embedded NUL is valid in a
string, but rejected where a filesystem path or numeric input must be a C string.

## Arrays, structures and ownership

```text
ST Person { age: int, names: [str] }
LT a: Person = Person { age: 25, names: ["Ada"] }~
LT b: Person = a~
b.names[0] = "Grace"~
println(a.names[0])~
```

The example prints `Ada`. Assignment and passing values do not expose mutable
aliases. Dynamic data is reference-counted, and collection writes copy shared
storage. `push(array, value)` returns a new array value; assign the result to grow
a variable. Arrays use zero-based, checked indexing. Strings do not support indexing;
`len(str)` counts Unicode scalars, `byte_len(str)` counts UTF-8 bytes.

Struct literals initialize every field exactly once, in any order. Fields are
accessible through `.name`. Empty structs are supported. Recursive struct-type
graphs, including recursion through an array, are currently rejected. There are
no user pointers or reference cycles. Owned values are released on scope exit,
return, break and continue; the runtime verifies ownership at normal program exit.
Fatal runtime errors free tracked dynamic objects before terminating the process.

## Modules

```text
IM "math.plr" AS math~
IM "std/io" AS io~
FN main(): int { io::println(math::twice(21))~ RET 0~ }
```

Paths resolve relative to the importing file. An alias is mandatory. Imports are
module-level and name `.plr` files; duplicate aliases and cyclic imports are errors.
Use `PUB FN` and `PUB ST` to export declarations; other declarations are private.
Function and struct names cannot collide within a module. Importing a module once
under multiple aliases does not duplicate its definitions. Standard modules are
embedded compiler/runtime capabilities and do not depend on files in the cwd.

## Standard library

| Module | Functions |
| --- | --- |
| `std/io` | `print(value)`, `println(value)`, `read_int()`, `read_float()`, `read_line()` |
| `std/math` | `abs(number)`, `sqrt(float)`, `pow(float, float)` |
| `std/str` | `len(str)`, `byte_len(str)`, `int(str)`, `float(str)` |
| `std/array` | `len(array)`, `push(array, value)` |
| `std/env` | `args(): [str]`, excluding the executable name |
| `std/fs` | `read_file(path): str`, `write_file(path, text): unit` |

These functions also have unqualified builtin spellings; a user-defined function
of the same name takes precedence. `print` accepts int, float, bool and str without
a newline; `println` adds one. Float output uses six significant digits (`%g`).

`OUTI value~`, `OUTF value~`, `OUTS value~` are typed newline-output sugar.
`INPI name~`, `INPF name~`, `INPS name~` assign the corresponding read result to
an existing local of the correct type.

Numeric reads consume a whitespace-delimited token, including its first trailing
whitespace character; `read_line` reads the remaining line without LF/CRLF. EOF,
malformed numeric input, invalid UTF-8 and I/O failures are distinct runtime errors,
never a fabricated zero. File operations use UTF-8 text and binary filesystem I/O;
Windows paths and arguments are converted to/from Unicode.

## Errors and compatibility

Diagnostics include a stable code, severity, source path and physical segments.
Runtime failures report `path:row:column: error[Rxxxx]` and a nonzero exit status.
Compiler optimization changes performance, not safety checks or observable rules.
This alpha has breaking changes from 1.x; see MIGRATION.md. JSON diagnostic schema
version 1 is an integration contract. AST/HIR/IR dumps are development interfaces.
