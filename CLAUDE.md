# palette

## Architecture

See [docs/guides/palette-design.md](docs/guides/palette-design.md) for the crate dependency graph, layer responsibilities, and design principles.

## Build & Check

Before committing, always run the following commands:

```bash
export RUST_MIN_STACK=33554432
cargo fmt
cargo clippy
cargo test
```

The `RUST_MIN_STACK` setting is required to work around a local rustc SIGSEGV issue (LLVM crash during compilation). Always set it before running cargo commands.

Fix any warnings or errors before pushing.
