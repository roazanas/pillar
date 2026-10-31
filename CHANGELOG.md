# Changelog

## 2.0.0-alpha.1 — unreleased

- Add reusable compiler analysis with physical source maps and structured diagnostics.
- Enforce types, lexical scope, signatures, typed returns, bool conditions and checked arithmetic.
- Add strings, Unicode escapes, arrays, value-semantic structures, imports and standard modules.
- Add short-circuit logic, loop exits, explicit conversions, file I/O and program arguments.
- Embed the native C runtime; use isolated linking and atomic executable replacement.
- Add check/build/run/fmt/transpose/completions commands, JSON diagnostics and stdio LSP.
- Add column editing and preview in a VS Code extension and local Docker playground.
- Add native CI, regression tests, archive smoke tests, sanitizers and benchmark tooling.
- Add MIT licensing and migration/reference documentation; update dependencies and Rust.

This is a development prerelease. Public release readiness requires the native CI
matrix and real-container tests; local macOS checks alone do not establish it.
