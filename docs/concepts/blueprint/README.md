# Blueprint

## Definition

A Blueprint is a document that defines a [Task](../task/) tree. It describes Tasks, their child Tasks, dependencies between sibling Tasks, and [Jobs](../job/) assigned to Tasks. Tasks can be nested to any depth.

A Blueprint is produced by a [Crafter](../worker/member/crafter/) as the deliverable of a planning Task. For example, the [Operator](../operator/) gives Palette a goal such as "add feature X." Palette assigns a Crafter to plan that goal, and the Crafter produces a Blueprint that breaks it down into concrete child Tasks.

A Blueprint is a static definition — it describes *what* should be done, not the state of ongoing work. When a Blueprint is used to start a [Workflow](../workflow/), the Workflow tracks the execution state separately.

## Examples

```yaml
task:
  id: 2026/feature-x
  title: Add feature X

children:
  - id: planning
    children:
      - id: api-plan
        type: craft
        plan_path: 2026/feature-x/planning/api-plan
      - id: api-plan-review
        type: review
        depends_on: [api-plan]

  - id: execution
    depends_on: [planning]
    children:
      - id: api-impl
        type: craft
        plan_path: 2026/feature-x/execution/api-impl
        repository:
          name: x7c1/palette
          branch: feature/x-api-impl
      - id: api-impl-review
        type: review
        depends_on: [api-impl]
```

## Collocations

- produce (a Blueprint as the deliverable of a planning Task)
- review (a Blueprint for quality and completeness)
- parse (a Blueprint from YAML)

## Domain Rules

- A Blueprint must contain exactly one root Task.
- A Blueprint is the source of truth for the Task tree structure.

## Related Concepts

- [Task](../task/) — the goals that the Blueprint defines
- [Job](../job/) — the work assignments that the Blueprint specifies
- [Workflow](../workflow/) — an execution of a Blueprint
- [Crafter](../worker/member/crafter/) — produces a Blueprint as a planning deliverable
- [Reviewer](../worker/member/reviewer/) — reviews a Blueprint
- [Plan](../plan/) — the documents that Tasks and Jobs reference via `plan_path`
