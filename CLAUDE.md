# palette

## Overview

@README.md

## Architecture

See [docs/guides/palette-design.md](docs/guides/palette-design.md) for the crate dependency graph, layer responsibilities, and design principles.

## Build & Check

Before committing, always run the following commands:

```bash
cargo fmt
cargo clippy
cargo test
```

`.cargo/config.toml` sets `-D warnings` globally, so all warnings are treated as errors in build, test, and clippy.
