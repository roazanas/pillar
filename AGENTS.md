# Working on Pillar

- Use GitHub account `roazanas` for this repository. Verify the active account before GitHub operations.
- Git author and committer must be `roazanas <me@rznz.ru>`; set these locally, not globally.
- Keep one Rust crate with a reusable library and a thin CLI.
- Keep the pipeline explicit: geometry, tokens, AST, types, checked IR, native code.
- Return structured diagnostics from library code; printing and exit belong to the CLI.
- Put build outputs in `OUT_DIR` or `target/`; never commit binaries or host-specific paths.
- Treat `.plr` as source and keep `.rplr` views in sync using `plrc transpose` and `plrc fmt`.
- Add a regression test for language changes. Execution tests must have time and output limits.
- Preserve runtime checks under optimization and release owned values on every exit path.
- Keep documentation plain and concise. Update the reference and migration notes for syntax changes.

Run before committing:

```sh
cargo fmt --all -- --check
cargo clippy --all-targets --all-features --locked -- -D warnings
cargo test --all-targets --all-features --locked
```

Editor, playground, fuzzing, sanitizers and packaging checks are in
[docs/VALIDATION.md](docs/VALIDATION.md). Use Docker for playground execution.
Release readiness requires all five native CI targets; local checks alone are insufficient.
