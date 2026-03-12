# Blueprint

## Definition

A Blueprint is a document that defines a [Task](../task/) and the [Jobs](../job/) needed to accomplish it. It is the input that an [Operator](../operator/) provides to Palette to start work.

A Blueprint contains a Task identity (id, title, and [Plan](../plan/) location) and a list of Job entries. Each Job entry specifies the Job type (craft or review), its dependencies on other Jobs, its Plan location, and typically a repository and branch to work on.

## Lifecycle

1. **Submit**: The Operator submits a Blueprint as a YAML document. Palette stores it for later use.
2. **Load**: The Operator loads a stored Blueprint. Palette creates the Jobs defined in the Blueprint and transitions them according to the lifecycle rules.

Submitting a Blueprint does not create Jobs — it only stores the document. Loading is the step that instantiates the Jobs and begins execution.

## Examples

```yaml
task:
  id: 2026/feature-x
  title: Add feature X
  plan_path: 2026/feature-x

jobs:
  - id: C-A
    type: craft
    title: Implement API
    plan_path: 2026/feature-x/api-impl
    priority: high
    repository:
      name: x7c1/palette
      branch: feature/test

  - id: R-A
    type: review
    title: Review API
    plan_path: 2026/feature-x/api-review
    depends_on: [C-A]
```

## Collocations

- submit (a Blueprint to store it)
- load (a Blueprint to create Jobs from it)
- parse (a Blueprint from YAML)

## Domain Rules

- A Blueprint must contain exactly one Task identity.
- A Blueprint must contain at least one Job entry.
- Each Job entry with a repository must specify a branch.
- Loading a Blueprint creates all Jobs as draft, then transitions eligible Craft Jobs to ready.

## Related Concepts

- [Task](../task/) — the goal that the Blueprint defines
- [Job](../job/) — the units of work that the Blueprint specifies
- [Operator](../operator/) — submits and loads Blueprints
- [Plan](../plan/) — the documents that Task and Jobs reference via `plan_path`
- [Orchestrator](../orchestrator/) — processes the Jobs created from a Blueprint
