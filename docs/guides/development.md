# Development Guide

## Build & Check

Before committing, run:

```bash
cargo fmt
cargo clippy
cargo test
```

`.cargo/config.toml` sets `-D warnings` globally, so all warnings are treated as errors in build, test, and clippy.

## Configuration

Palette resolves its configuration file as follows:

- `--config <path>` — use the specified file
- No `--config` — use `~/.config/palette/config.toml`

The repository ships a development config at `config/palette.toml`, which uses relative paths (e.g., `db_path = "data/palette.db"`). This means commands run with `--config config/palette.toml` operate on the local `data/` directory within the repository.

Without `--config`, commands target the user config at `~/.config/palette/`, which may contain production data.

## Running Commands Locally

Start the Orchestrator with the development config:

```bash
cargo run --bin palette -- start --config config/palette.toml
```

Admin commands also require an explicit config to avoid touching production data:

```bash
cargo run --bin palette -- admin gc --config config/palette.toml --dry-run
cargo run --bin palette -- admin reset --config config/palette.toml --dry-run
```

## Architecture

See [palette-design.md](palette-design.md) for the crate dependency graph, layer responsibilities, and design principles.
