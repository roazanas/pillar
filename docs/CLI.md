# Command-line guide

```text
plrc check FILE
plrc build FILE [-o PATH] [--release] [--linker PATH]
plrc run FILE [-- PROGRAM_ARGS...]
plrc fmt FILE [--check | --write]
plrc transpose FILE --from horizontal|vertical
plrc completions bash|zsh|fish|powershell|elvish
plrc lsp
```

`plrc --version` reports the package version and embedded native target; archive
packaging verifies both.

`check` parses, resolves imports and checks semantics without calling a linker.
Modules without `main` may be checked. `build` defaults to `out` (`out.exe` on
Windows). `run` builds into a temporary directory, inherits the current working
directory and standard streams, forwards arguments and returns the program status.

`--release` enables Cranelift optimization; integer safety checks remain enabled.
`--linker PATH` selects a native toolchain driver: cc-compatible on Unix, cl-compatible
on Windows/MSVC. Otherwise `PILLAR_LINKER`, then `CC`, then the platform default is
used. Windows auto-detection locates installed Visual Studio tools and SDK libraries.
This is a native compiler: overriding a driver does not enable cross-compilation.

Builds use a separate temporary directory and atomically replace the output only
after linking succeeds. Existing output survives failures; source files cannot be
used as output paths. The embedded runtime archive matches the compiler's target.
Builds do not read a `runtime.o` from the working directory.

`build --emit=tokens|ast|hir|ir|obj|exe` selects a development dump or native output.
Textual dumps go to stdout; object output defaults to `out.o`. Native output requires
a root `main`; check and frontend dumps may operate on a library module.

`fmt` writes canonical vertical source to stdout by default. `--write` performs an
atomic replacement; `--check` returns 1 when formatting is needed, without writing.
Syntax/lexical errors prevent formatting. Formatting can operate on type-invalid
code and does not require a linker. It preserves token values and comment contents,
allowing grid padding at comment line ends. Formatting is idempotent.

`transpose --from horizontal` imports a horizontal view with a spacer column between
logical lines. `--from vertical` exports the logical text, trimming grid padding.
Formatting is a separate operation. Input remains UTF-8 and is subject to geometry
limits. The examples' `.rplr` views are checked for token equivalence in tests.

`--diagnostic-format=human|json`, `--color=auto|always|never`, and `-v/--verbose` are
global options. Diagnostics go to stderr; dumps/completions and program output go to
stdout. Human color respects `NO_COLOR` in auto mode. JSON diagnostics are one object
per line, use `schema_version: 1`, stable code/severity/message/file fields and a
`labels` array of physical start/end positions. Positions are 1-based scalar row/column
and zero-based UTF-8 byte offsets. Exceptionally large labels are capped at 256
segments and marked `labels_truncated`; the library SourceMap retains the full range. Do not enable verbose logging when parsing stderr
as JSON. Clap argument errors have their own CLI usage format and exit status 2.

`plrc FILE [-o PATH]` and `plrc -t FILE` remain compatibility aliases for build and
horizontal import. `.rplr` is not accepted by build/check/run. LSP uses Content-Length
framing on stdio and zero-based UTF-16 positions, not the CLI coordinate convention.
