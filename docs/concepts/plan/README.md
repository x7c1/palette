# Plan

## Definition

A Plan is a document that describes what should be accomplished and how. Plans are organized in a directory hierarchy and can exist at any level — for a [Task](../task/) (describing overall scope and approach) or for a [Job](../job/) (describing specific work to perform).

The directory hierarchy of Plans is independent of the Job structure. Jobs are always flat, while Plans can be nested freely. A Job references its Plan via a `plan_path`.

## Location

Plans are stored under a system-wide `plan_dir` setting defined in `config/palette.toml`. Each Job carries a `plan_path` that specifies where its Plan lives relative to `plan_dir`.

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

## Lifecycle

1. **Planning phase**: The [Leader](../worker/supervisor/leader/) determines the Job structure and generates a planning-phase [Blueprint](../blueprint/). [Crafters](../worker/member/crafter/) create Plans and [Reviewers](../worker/member/reviewer/) review them.
2. **Execution phase**: Workers receive the `plan_path` as part of their Job instruction and follow the Plan to complete the Job.

## Splitting work

When a Job turns out to be larger than expected, the Worker completes it at a natural stopping point (e.g., where the build and tests pass) and the remaining work is added as new Jobs with their own Plans.

For example, if Job `api-impl` is too large:

1. The Worker finishes `api-impl` at a clean boundary
2. New Jobs are created for the remaining work (e.g., `api-impl-auth`, `api-impl-endpoints`)
3. Each new Job has its own `plan_path`, which can nest under the original Plan's directory:

```
docs/plans/2026/feature-x/
  api-impl/
    README.md                  ← Original Plan (completed)
    api-impl-auth/
      README.md                ← Remaining work: auth
    api-impl-endpoints/
      README.md                ← Remaining work: endpoints
```

Jobs remain flat in the [Blueprint](../blueprint/). The Plan hierarchy is purely a filesystem concern.

## Collocations

- create (a Plan for a Job)
- review (a Plan for quality and completeness)
- follow (a Plan during execution)
- split (remaining work into new Jobs with their own Plans)

## Domain Rules

- A Job references its Plan via `plan_path`.
- A Plan must be reviewed and approved before its corresponding Job enters the execution phase.
- Plans can be nested in the filesystem without affecting the Job structure.
- When work is split, the original Job is completed and new Jobs are created — Jobs are never cancelled due to scope.

## Related Concepts

- [Job](../job/) — the unit of work that the Plan describes
- [Blueprint](../blueprint/) — defines the Task and Jobs; the planning-phase Blueprint triggers Plan creation
- [Crafter](../worker/member/crafter/) — creates the Plan during the planning phase, follows it during the execution phase
- [Reviewer](../worker/member/reviewer/) — reviews the Plan for quality
- [Leader](../worker/supervisor/leader/) — determines Job structure and initiates Plan creation
