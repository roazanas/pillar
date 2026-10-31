# Migrate 1.x programs to 2.0 alpha

This development version changes language semantics deliberately. Keep a copy of
existing programs before migrating; no old-language compatibility parser is provided.

1. Declare result types: `FN main(): int`, `FN factorial(n: int): int`. Omission now
   means unit, and unit functions may use `RET~` or fall through.
2. Correct variable annotations. `LT x: int = 1.5~` is now an error; use a float or an
   explicit conversion. Assignments and arguments have matching types.
3. Use bool conditions and logical not. Integers are not truthy, and `!2` is rejected.
   Arithmetic overflow, invalid input and zero division are errors in optimized code too.
4. Respect lexical scopes: a name declared inside an IF/WH is unavailable outside.
   Shadowing a name does not replace the outer binding. Declare outer state before a
   branch if it must be assigned in several branches.
5. Write `EL { ... }` as a complete pair. Statements end with `~`; `;` is not accepted.
   Parenthesize comparisons instead of writing chains. Remove unreachable statements;
   a literal-true loop without a break has no fallthrough.
6. Build vertical `.plr` files. Import `.rplr` views using `plrc transpose --from
   horizontal`. Examples have moved from the repository root to `examples/`.
7. Remove local `runtime.o` workarounds. The compiler embeds a native runtime archive
   and invokes a native linker independently of the working directory. Windows uses
   MSVC C++ Build Tools and Windows SDK rather than MinGW.
8. Prefer explicit CLI commands. Legacy file/-t forms still work temporarily.
   Build success is quiet; diagnostics are on stderr. Rust users can reuse library
   analysis and code-generation APIs instead of shelling out to the old main function.

Packaging now uses native archives with checksums, manifests, documentation and
completions. The previous generated cargo-dist/MSI configuration is superseded;
older published installers refer to older compiler versions. No new release or
extension publication is performed merely by editing this repository.
