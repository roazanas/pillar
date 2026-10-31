CI runs native compiler, CLI and LSP regressions on the five supported targets,
then packages and executes the extracted compiler in a clean working directory.
The tools job tests grid editing, the playground HTTP boundary and real Docker limits.

A `v<VERSION>` tag runs the same complete workflow before publishing native
`.tar.gz`/`.zip` archives with checksums. The tag must match Cargo.toml. Prerelease
versions are marked as GitHub prereleases. Publishing requires all jobs to pass.
The older cargo-dist/MSI setup is superseded by tested native archives; existing
published installers are historical and do not describe this development version.
