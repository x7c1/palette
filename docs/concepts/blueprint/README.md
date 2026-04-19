# Blueprint

## Definition

A Blueprint is a document that defines a [Task](../task/) tree. It describes Tasks, their child Tasks, dependencies between sibling Tasks, and [Jobs](../job/) assigned to Tasks. Tasks can be nested to any depth.

A Blueprint is produced by a [Crafter](../worker/member/crafter/) as the deliverable of a planning Task. For example, the [Operator](../operator/) gives Palette a goal such as "add feature X." Palette assigns a Crafter to plan that goal, and the Crafter produces a Blueprint that breaks it down into concrete child Tasks.

A Blueprint is a static definition — it describes *what* should be done, not the state of ongoing work. When a Blueprint is used to start a [Workflow](../workflow/), the Workflow tracks the execution state separately.

A Blueprint can be edited while the Workflow is suspended. The [Operator](../operator/) edits the Blueprint, then applies the changes. Applying validates the edits against change rules and reconciles the differences with the Workflow's state in the database.

## Co-location with Plans

A Blueprint is stored as `blueprint.yaml`. Any Task in the tree — including the root — may carry a `plan_path` pointing to a [Plan](../plan/) document. `plan_path` is resolved as a relative path from the Blueprint's directory, so the Blueprint and every Plan it references form a single co-located directory subtree.

When the root Task carries `plan_path`, that Plan articulates the workflow's overall purpose and scope; when child Tasks carry `plan_path`, each Plan describes its Task's specific work.

```
<work-unit-dir>/
  blueprint.yaml         ← Blueprint (this Task tree)
  README.md              ← Plan for the root Task (purpose, scope)
  <child-dir>/
    README.md            ← Plan for a child Task
```

Rules:

- A `README.md` must exist alongside every `blueprint.yaml` as the Blueprint's parent plan. Palette's parser rejects a Blueprint whose directory has no `README.md`.
- Every `plan_path` declared in the Blueprint must point to an existing file under the Blueprint's directory. Palette's parser rejects the Blueprint otherwise.
- Only one Blueprint per work unit; nested `blueprint.yaml` in a subdirectory is rejected.
- All `plan_path` values are relative to the Blueprint's directory. Absolute paths and `..` are rejected.

## Examples

```yaml
task:
  key: feature-x
  plan_path: README.md           # plan for the whole workflow
  children:
    - key: planning
      children:
        - key: api-plan
          type: craft
          plan_path: planning/api-plan/README.md
          repository:
            name: x7c1/palette
            branch: feature/x-api-plan
          children:
            - key: api-plan-review
              type: review

    - key: execution
      depends_on: [planning]
      children:
        - key: api-impl
          type: craft
          plan_path: execution/api-impl/README.md
          repository:
            name: x7c1/palette
            branch: feature/x-api-impl
          children:
            - key: api-impl-review
              type: review
```

Every `craft` Task must have a `review` child — Palette rejects a Blueprint whose `craft` Task has no review. The review runs after its parent `craft` completes, so the ordering is implicit and no `depends_on` is needed for the review.

## Repository Fields

`repository:` on a `craft` Task specifies where the work lands:

| Field | Meaning | Default |
|---|---|---|
| `name` | `owner/repo` slug | required |
| `branch` | Work branch Palette creates and commits to. Does not need to exist on origin yet. | required (typically `feature/<craft-key>`) |
| `source_branch` | Branch to derive `branch` from when it does not exist on origin. Ignored for the resume path (`branch` already on origin). | repository default branch (`refs/remotes/origin/HEAD`) |

The orchestrator creates the work branch; the Crafter does not. When the same `(name, branch)` pair is already in use by another non-terminal [Workflow](../workflow/), `POST /workflows/start` rejects the request with `workflow/branch_in_use` so that two workflows never contend for the same branch.

## Blueprint Location Modes

The orchestrator auto-detects where the Blueprint sits relative to the Craft workspace and picks one of two modes at workspace-creation time:

- **Repo-inside-Plan**: the Blueprint directory is a subdirectory of the target repo. The Blueprint and Plan files are committed on the work branch alongside the Craft output, so PR reviewers see plan and code together. Relative links inside the Plan can reach the whole workspace.
- **Repo-outside-Plan**: the Blueprint lives in a different repo (e.g. a shared workspace repo that coordinates plans for other projects). The Blueprint directory is bind-mounted read-only under `/home/agent/plans`; the workspace's git history stays free of Plan files. Relative links inside the Plan resolve only within the Blueprint directory.

Mode detection compares the Blueprint's absolute host path against the workspace's absolute host path — no configuration needed.

## Validation

Palette exposes `POST /blueprints/validate` so a Blueprint can be checked before any Workflow is created. The endpoint reads the file, runs the same schema and structural validation as `POST /workflows/start`, and returns either a summary of the parsed tree (when valid) or a list of machine-readable errors (when invalid). The endpoint is side-effect-free — no database rows, no network calls, no activation events — so it is safe to call as a pre-flight from skills (`/palette:plan` uses it after generation; `/palette:approve` runs it before starting a Workflow).

When invalid, each entry in `errors[]` carries a `location`, a `hint` pointing at the offending field, and a `reason` code in `{namespace}/{value}` form (e.g. `blueprint/missing_review_child`, `blueprint/plan_path_missing`, `invalid_task_key/invalid_format`). Clients can branch on `reason` without string-parsing free-form messages.

## Collocations

- produce (a Blueprint as the deliverable of a planning Task)
- review (a Blueprint for quality and completeness)
- parse (a Blueprint from YAML)
- edit (a Blueprint while the Workflow is suspended)
- apply (a Blueprint change to reconcile with the Workflow's state)

## Domain Rules

- A Blueprint must contain exactly one root Task.
- A Blueprint is the source of truth for the Task tree structure.
- A Blueprint can only be edited while the Workflow is suspended.
- Edits are restricted to Tasks that are Pending or Ready. Tasks that are Completed, InProgress, or Suspended — and their subtrees — cannot be modified.

## Related Concepts

- [Task](../task/) — the goals that the Blueprint defines
- [Job](../job/) — the work assignments that the Blueprint specifies
- [Workflow](../workflow/) — an execution of a Blueprint
- [Crafter](../worker/member/crafter/) — produces a Blueprint as a planning deliverable
- [Reviewer](../worker/member/reviewer/) — reviews a Blueprint
- [Plan](../plan/) — the documents that Tasks and Jobs reference via `plan_path`
