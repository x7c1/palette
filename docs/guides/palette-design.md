# Palette Design

## Crate Dependency Graph

```
palette-domain       (no dependencies — pure domain types)
palette-tmux         (no dependencies — terminal operations)
palette-db           → palette-domain
palette-docker       → palette-domain
palette-file-state   → palette-domain
palette-fs           → palette-domain
palette-orchestrator → palette-db, palette-docker, palette-domain, palette-file-state, palette-tmux
palette-server       → palette-db, palette-docker, palette-domain, palette-fs, palette-tmux
palette-cli          → palette-db, palette-docker, palette-domain, palette-file-state, palette-orchestrator, palette-server, palette-tmux
```

```mermaid
graph TD
    cli[palette-cli] --> server[palette-server]
    cli --> orch[palette-orchestrator]
    cli --> docker[palette-docker]
    cli --> file[palette-file-state]
    cli --> db[palette-db]
    cli --> domain[palette-domain]
    cli --> tmux[palette-tmux]

    server --> db
    server --> docker
    server --> domain
    server --> fs[palette-fs]
    server --> tmux

    orch --> db
    orch --> docker
    orch --> domain
    orch --> file
    orch --> tmux

    fs --> domain
    file --> domain
    db --> domain
    docker --> domain
```

Note: palette-server depends on palette-orchestrator only as a dev-dependency (for integration tests), not in production code.

## Layer Responsibilities

| Crate | Role |
|---|---|
| palette-domain | Pure domain types (Task, Job, Workflow, etc.). No serde, no I/O, no external format dependencies. |
| palette-db | Database access. Owns DB-specific types (e.g. TaskRow). Implements domain traits (TaskStore, JobStore). |
| palette-fs | Filesystem access. Reads Blueprint YAML files and converts to domain types. Owns YAML deserialization types. |
| palette-file-state | Persists runtime state (PersistentState) to JSON files. |
| palette-docker | Docker container management. |
| palette-tmux | Terminal (tmux) session management. |
| palette-orchestrator | Orchestration logic. Processes rule engine effects, manages worker lifecycle. |
| palette-server | HTTP API layer. Owns API request/response types. Routes and handlers. |
| palette-cli | Entry point. Configuration loading, server startup. |

## Design Principles

- **palette-domain has no external format dependencies.** Each layer defines its own serialization types and converts to/from domain types. Do not add serde to palette-domain.
- **Types are not shared across layers.** A YAML type in palette-fs, a DB row type in palette-db, and an API type in palette-server may represent the same concept but are distinct types with distinct responsibilities. These are not "intermediate types" — they are the appropriate representation for each layer.
- **Dependencies flow inward.** All crates depend on palette-domain. Infrastructure crates (db, fs, docker, tmux) do not depend on each other. palette-server does not depend on palette-orchestrator in production.
