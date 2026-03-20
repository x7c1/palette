---
description: Start Palette (server + orchestrator + supervisors)
---

# palette-start

Start Palette for local development and E2E testing. This launches the HTTP server, orchestrator event loop, and supervisor containers.

## Instructions

- Run `scripts/reset.sh` to clean up any previous state (containers, tmux session, DB files)
- Build Palette with `cargo build`
  - Set `RUST_MIN_STACK=33554432` to avoid rustc SIGSEGV
- Start Palette in the background with `cargo run &`
- Capture the PID and write it to `data/palette.pid`
- Health-check by polling `curl -sf http://127.0.0.1:7100/jobs` every 2 seconds, up to 60 seconds total
- Once the health check passes, report that Palette is running and show the PID
- If the health check times out after 60 seconds, report failure and show the last few lines of output

## Notes

- `cargo run` starts the server (port 7100), orchestrator, and bootstraps supervisor containers (leader + review integrator) via Docker
- Configuration is in `config/palette.toml`
- The working directory must be the Palette repository root
- `data/palette.pid` is used by `palette-stop` to shut down the process
