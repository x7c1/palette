---
description: Create a Blueprint YAML and parent plan from a task description
argument-hint: <slug>
---

# palette-plan

Create a Blueprint YAML and its parent plan (`README.md`) from a task description provided by the Operator. The two files are co-located in a single directory by convention.

## Arguments

- `$0`: Short kebab-case slug describing the work (e.g., `add-user-auth`). Used in the output directory name.

## Instructions

- Ask the Operator where to put the new plan. Offer two options:
  - **A. Inside the Palette workflow's target repo (workspace)** — for plans that ship together with the code being changed (the common case).
  - **B. Inside the current CWD's repo** — for plans managed in an external repository (for example, managing palette's own plans from atelier).
- After the Operator picks the base, compute the default save directory:
  ```
  <base>/docs/plans/<YYYY>/<MMDD>-<slug>/
  ```
  - `<YYYY>` is the current year (e.g., `2026`).
  - `<MMDD>` is the current month and day (e.g., `0417`).
  - `<slug>` is `$0`.
  - Example: `docs/plans/2026/0417-add-user-auth/`
- Confirm the path with the Operator (allow override).
- Ask the Operator to describe the task and its subtasks.
- Generate the Blueprint YAML in the chosen directory as `blueprint.yaml`:
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
  - `plan_path:` is **relative to the Blueprint directory**. Absolute paths and `..` are rejected.
- Generate the parent plan as `README.md` in the same directory. Include:
  - The work's purpose and scope
  - A brief overview of the subtasks
  - Any constraints or success criteria
- Display both generated files to the Operator for review.
- Apply any requested modifications.
- Once the Operator approves, inform them to run `palette-approve` with the **absolute path** to `blueprint.yaml` to start the Workflow.

## Notes

- The co-location convention (parent `README.md` next to `blueprint.yaml`) is enforced by Palette's Blueprint parser. Skipping the README causes workflow start to fail.
- In the future, a Crafter agent will generate Blueprints automatically. This skill serves as a manual bridge until that automation is ready.
