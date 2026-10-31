# Validation and development

Use the pinned Rust toolchain and a native C toolchain. All commands below are run
from the repository root unless noted. Local overrides such as `SDKROOT` remain
ordinary toolchain environment settings; they are not baked into the compiler.

```sh
cargo fmt --all -- --check
cargo clippy --all-targets --all-features --locked -- -D warnings
cargo test --all-targets --locked
cargo run --locked --features fuzzing --bin plrc-fuzz -- 10000
node --test editor/columns.test.js
python3 -m unittest discover -s playground -p 'test_*.py'
```

The Rust integration tests compile and run real native programs. They cover the
original examples, typed calls, recursion, scopes, control flow, checked arithmetic
in both optimization modes, Unicode, strings, collection ownership, imports,
CLI output atomicity, diagnostics, formatting and LSP framing/coordinates/rename.
Executables use separate temporary directories and have execution deadlines.
Horizontal example views are checked against canonical `.plr` tokens.

The optional `fuzzing` feature adds `plrc-fuzz`, a deterministic mutation runner
seeded with valid grammar. Its optional second argument is a seed. It checks both
frontend survival and formatter idempotence for valid programs. Failures write
`target/fuzz-crash.plr` and exit nonzero. This bounded regression campaign complements
rather than replaces long-running coverage-guided fuzzing.

## Memory safety

```sh
cargo build --locked --bin plrc
python3 tools/check_memory.py --compiler target/debug/plrc --cc clang
```

The tool emits real Cranelift objects and links them to a separate ASan/UBSan
instrumented runtime. It checks nested references, copy-on-write, scope/loop/return
cleanup and fatal input/arithmetic errors. Linux also enables LeakSanitizer; macOS
uses ASan/UBSan and the runtime's owned-object invariant. The standard Windows job
uses the regular native ownership tests, not Unix sanitizer flags.

## Distribution

```sh
cargo build --release --locked --bin plrc
python3 tools/package.py --binary target/release/plrc --target aarch64-apple-darwin
```

Choose the target matching the native compiler; on Windows use `plrc.exe` and the
Windows target. The script generates completions, a content/hash manifest, third-party notices and an
archive checksum. It extracts into a new directory with spaces, verifies every
manifest hash and runs all five original examples from an empty working directory.
Packaging is deterministic for the same input files and compiler binary.

CI runs this procedure natively on five platform/architecture pairs. A tagged
release depends on all jobs, including real Docker sandbox smoke tests. Release
publication is a separate action triggered by an explicit matching version tag.

## Measurements

```sh
python3 tools/bench.py --compiler target/release/plrc > target/benchmark.json
```

The fixed 10/100/1000-function workloads measure median check, optimized-build and
native-process wall time, source size and binary size. Each executable also runs
a checked one-million-iteration sum loop; its output is verified. Unix records cumulative
peak child RSS for the run (bytes; not a per-stage allocation measurement).
Windows reports that field as null. Runtime timings include process startup.
Record compiler/version/host alongside results; compare like environments before
changing optimization policy.
The initial local sample is recorded in [benchmark-macos-arm64.json](benchmark-macos-arm64.json).

## Editor and playground

```sh
(cd editor/vscode && npm ci --ignore-scripts && npm run check && npm test)
docker build -f playground/Dockerfile -t pillar-playground:local .
python3 playground/smoke.py
```

Open the VS Code extension development host to verify actual editor integration.
Protocol and grid tests alone do not establish GUI compatibility. See the
playground README for the loopback-only server and execution limits. Without a
running Docker engine, preview-only mode serves the UI and refuses every compiler
or execution request. Never substitute host execution for a missing sandbox.
