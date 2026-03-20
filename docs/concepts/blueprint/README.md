# Blueprint

## Definition

A Blueprint is a document that defines a [Task](../task/) tree. It describes Tasks, their child Tasks, dependencies between sibling Tasks, and [Jobs](../job/) assigned to Tasks. Tasks can be nested to any depth.

A Blueprint is produced by a [Crafter](../worker/member/crafter/) as the deliverable of a planning Task. For example, the [Operator](../operator/) gives Palette a goal such as "add feature X." Palette assigns a Crafter to plan that goal, and the Crafter produces a Blueprint that breaks it down into concrete child Tasks.

## Lifecycle

1. **Submit**: A Blueprint is submitted as a YAML document. Palette stores it for later use.
2. **Load**: A stored Blueprint is loaded. Palette creates the Tasks and Jobs defined in the Blueprint and begins execution.

Submitting a Blueprint does not create Tasks or Jobs — it only stores the document. Loading is the step that instantiates them and begins execution.

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
- submit (a Blueprint to store it)
- load (a Blueprint to create Tasks and Jobs from it)
- parse (a Blueprint from YAML)

## Domain Rules

- A Blueprint must contain exactly one root Task.
- Loading a Blueprint creates the Task tree and Jobs defined in it.

## Related Concepts

- [Task](../task/) — the goals that the Blueprint defines
- [Job](../job/) — the work assignments that the Blueprint specifies
- [Crafter](../worker/member/crafter/) — produces a Blueprint as a planning deliverable
- [Plan](../plan/) — the documents that Tasks and Jobs reference via `plan_path`
- [Orchestrator](../orchestrator/) — processes the Tasks and Jobs created from a Blueprint
