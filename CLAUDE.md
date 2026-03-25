# palette

## Architecture

See [docs/guides/palette-design.md](docs/guides/palette-design.md) for the crate dependency graph, layer responsibilities, and design principles.

## Build & Check

Before committing, always run the following commands:

```bash
cargo fmt
cargo clippy -- -D warnings
cargo test
```

CI runs with `-D warnings`, so all warnings must be fixed before pushing.
