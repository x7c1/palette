# Blueprint

## Definition

A Blueprint is a document that defines a [Task](../task/) tree. It describes Tasks, their child Tasks, dependencies between sibling Tasks, and [Jobs](../job/) assigned to Tasks. Tasks can be nested to any depth.

A Blueprint is produced by a [Crafter](../worker/member/crafter/) as the deliverable of a planning Task. For example, the [Operator](../operator/) gives Palette a goal such as "add feature X." Palette assigns a Crafter to plan that goal, and the Crafter produces a Blueprint that breaks it down into concrete child Tasks.

A Blueprint is a static definition — it describes *what* should be done, not the state of ongoing work. When a Blueprint is used to start a [Workflow](../workflow/), the Workflow tracks the execution state separately.

A Blueprint can be edited while the Workflow is suspended. The [Operator](../operator/) edits the Blueprint, then applies the changes. Applying validates the edits against change rules and reconciles the differences with the Workflow's state in the database.

## Co-location with Parent Plan

A Blueprint is stored as `blueprint.yaml` and must live alongside a parent [Plan](../plan/) (`README.md`) in the same directory. The parent plan describes the work unit's purpose and scope; the Blueprint defines its Task tree. Child [Plans](../plan/) referenced by `plan_path` live under this same directory as relative paths.

```
<work-unit-dir>/
  README.md              ← parent Plan (scope, purpose)
  blueprint.yaml         ← Blueprint (this Task tree)
  <child-dir>/
    README.md            ← child Plan referenced by plan_path
```

Rules:

- A Blueprint's directory must contain a sibling `README.md`. Palette's parser rejects a Blueprint without one.
- Only one Blueprint per work unit; nested `blueprint.yaml` in a subdirectory is rejected.
- All `plan_path` values are relative to the Blueprint's directory. Absolute paths and `..` are rejected.

## Examples

```yaml
task:
  key: feature-x
  children:
    - key: planning
      children:
        - key: api-plan
          type: craft
          plan_path: planning/api-plan/README.md
        - key: api-plan-review
          type: review
          depends_on: [api-plan]

    - key: execution
      depends_on: [planning]
      children:
        - key: api-impl
          type: craft
          plan_path: execution/api-impl/README.md
          repository:
            name: x7c1/palette
            branch: feature/x-api-impl
        - key: api-impl-review
          type: review
          depends_on: [api-impl]
```

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
