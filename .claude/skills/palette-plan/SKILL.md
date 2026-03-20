---
description: Create a Blueprint YAML from a task description
argument-hint: <task-id>
---

# palette-plan

Create a Blueprint YAML file from a task description provided by the Operator.

## Arguments

- `$0`: Task ID for the Blueprint (e.g., `2026/feature-x`). Used as the root task ID and part of the output filename.

## Instructions

- Ask the user to describe the task and its subtasks
- Generate a Blueprint YAML using the task tree format:
  ```yaml
  task:
    id: <task-id>
    title: <descriptive title>

  children:
    - id: <subtask-id>
      type: craft
      description: <what to do>
      # Optional fields: depends_on, priority, repository, plan_path
    - id: <subtask-id>
      type: review
      depends_on: [<previous-subtask>]
  ```
- Key rules for the YAML structure:
  - `task:` defines the root task with `id` and `title`
  - `children:` is a list of subtasks at the top level
  - Leaf tasks must have `type: craft` or `type: review`
  - Non-leaf tasks (composites) must NOT have a `type` field; they group child tasks via their own `children:` list
  - Use `depends_on:` to express ordering between sibling tasks
  - `priority:` can be `high`, `medium`, or `low`
  - `repository:` takes `name` and `branch` fields
- Save the file to `data/blueprints/<task-id-slug>.yaml` (replace `/` with `-` in task ID for filename)
- Display the generated YAML to the user and ask for review
- Apply any requested modifications
- Once the user approves, inform them to run `palette-approve` with the Blueprint path to start the Workflow

## Notes

- In the future, a Crafter agent will generate Blueprints automatically. This skill serves as a manual bridge until that automation is ready.
