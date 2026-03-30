# Plan

## Definition

A Plan is a document that describes what should be accomplished and how. Plans are organized in a directory hierarchy and can exist at any level — for a [Task](../task/) (describing overall scope and approach) or for a [Job](../job/) (describing specific work to perform).

The directory hierarchy of Plans is independent of the Task tree structure. A Task or Job references its Plan via a `plan_path`.

## Location

Plans are stored under a system-wide `plan_dir` setting defined in `config/palette.toml`. Each Task or Job carries a `plan_path` that specifies where its Plan lives relative to `plan_dir`.

For example, with `plan_dir = "docs/plans"`:

```
docs/plans/
  2026/
    feature-x/
      README.md            ← Task-level Plan (scope, approach)
      api-impl/
        README.md          ← Job api-impl's Plan
      api-spec/
        README.md          ← Job api-spec's Plan
```

A Job's `plan_path` might be `2026/feature-x/api-impl`, resolving to `docs/plans/2026/feature-x/api-impl/`.

## Splitting work

When a Task turns out to be larger than expected, the [Operator](../operator/) suspends the [Workflow](../workflow/), edits the [Blueprint](../blueprint/) to break the Task into child Tasks, and resumes. Each child Task has its own Plan.

For example, if Task `api-impl` is too large:

1. The Operator suspends the Workflow
2. The Operator edits the Blueprint, breaking `api-impl` into child Tasks (e.g., `api-impl-auth`, `api-impl-endpoints`)
3. Each child Task has its own `plan_path`, which can nest under the original Plan's directory:

```
docs/plans/2026/feature-x/
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

- A Task or Job references its Plan via `plan_path`.
- Plans can be nested in the filesystem without affecting the Task tree structure.

## Related Concepts

- [Task](../task/) — the goal that the Plan describes
- [Job](../job/) — the work assignment that the Plan describes
- [Blueprint](../blueprint/) — defines the Task tree; triggers Plan creation
- [Crafter](../worker/member/crafter/) — creates Plans and follows them during execution
- [Reviewer](../worker/member/reviewer/) — reviews Plans for quality
