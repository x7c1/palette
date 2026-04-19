# Plan

## Definition

A Plan is a document that describes what should be accomplished and how. Plans are organized in a directory hierarchy and can exist at any level — for a [Task](../task/) (describing overall scope and approach) or for a [Job](../job/) (describing specific work to perform).

The directory hierarchy of Plans is independent of the Task tree structure. A Task or Job references its Plan via `plan_path`.

## Location

Plans live under the directory of the [Blueprint](../blueprint/) that references them. Any Task in the Blueprint — including the root — may carry a `plan_path`, which is resolved as a relative path from the Blueprint's directory.

```
<blueprint-dir>/
  blueprint.yaml         ← Blueprint that defines the Task tree
  README.md              ← Plan for the root Task (scope, purpose)
  api-impl/
    README.md            ← Plan for child Task api-impl
  api-spec/
    README.md            ← Plan for child Task api-spec
```

`plan_path` values are **relative paths** from the Blueprint's directory. A child Task's `plan_path` might be `api-impl/README.md`, resolving to `<blueprint-dir>/api-impl/README.md`. Absolute paths, `..`, and scheme-prefixed strings (e.g. `plans://`, `repo://`) are rejected at parse time so plan resolution cannot escape the Blueprint's directory.

A Blueprint may declare no `plan_path` on any Task. This is valid and describes a purely mechanical workflow (such as an auto-generated PR review) whose intent is fully captured by the Task tree itself.

## Plan Delivery to Workers

The orchestrator makes Plans reachable to Workers in one of two modes, picked automatically from the host-side layout:

- **Repo-inside-Plan** (the Blueprint directory is under the target repo): the Blueprint is staged and committed on the work branch during workspace setup, so Plan files sit inside the Crafter's workspace. Plan paths in the instruction message resolve under `/home/agent/workspace/<blueprint-rel>/...`, and relative links inside the Plan can reach anything in the workspace.
- **Repo-outside-Plan** (the Blueprint lives outside the target repo, e.g. a separate workspace repo): the Blueprint directory is bind-mounted read-only at `/home/agent/plans`. Plan paths resolve under `/home/agent/plans/...`, and relative links resolve only within the Blueprint directory (the mount boundary).

The `Plan:` line in a Worker's first instruction is already a fully-resolved absolute container path — Workers read it verbatim regardless of mode. Mode detection happens at workspace-creation time by comparing the Blueprint's absolute host path with the workspace's absolute host path.

## Splitting work

When a Task turns out to be larger than expected, the [Operator](../operator/) suspends the [Workflow](../workflow/), edits the [Blueprint](../blueprint/) to break the Task into child Tasks, and resumes. Each child Task has its own Plan.

For example, if Task `api-impl` is too large:

1. The Operator suspends the Workflow
2. The Operator edits the Blueprint, breaking `api-impl` into child Tasks (e.g., `api-impl-auth`, `api-impl-endpoints`)
3. Each child Task has its own `plan_path`, which can nest under the original Plan's directory:

```
<blueprint-dir>/
  api-impl/
    README.md                  ← Original Plan (completed)
    api-impl-auth/
      README.md                ← Remaining work: auth
    api-impl-endpoints/
      README.md                ← Remaining work: endpoints
```

## Collocations

- create (a Plan for a Task or Job)
- review (a Plan for quality and completeness)
- follow (a Plan during execution)

## Domain Rules

- A Task or Job references its Plan via `plan_path`, resolved relative to the owning Blueprint's directory.
- Plans can be nested in the filesystem without affecting the Task tree structure.
- Every `plan_path` declared in a Blueprint must point to an existing file; a Blueprint with a dangling `plan_path` is rejected by the parser. A Blueprint that declares no `plan_path` at all is also valid.

## Related Concepts

- [Task](../task/) — the goal that the Plan describes
- [Job](../job/) — the work assignment that the Plan describes
- [Blueprint](../blueprint/) — defines the Task tree; triggers Plan creation
- [Crafter](../worker/member/crafter/) — creates Plans and follows them during execution
- [Reviewer](../worker/member/reviewer/) — reviews Plans for quality
