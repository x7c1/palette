---
name: plan
description: Interview the Operator about a new task and generate the Blueprint and plan documents
user-invocable: true
---

# /palette:plan

Interview the Operator about a new task, then generate a Blueprint YAML and its companion plan (`README.md`). The plan is referenced by the Blueprint's root task via `plan_path` and lives alongside `blueprint.yaml` in the same directory.

## Interaction principles

- **Ask one question at a time.** Wait for the Operator's answer before moving on. Do not batch multiple questions in a single message.
- **Let the Operator describe the work first.** The slug and directory name are derived from that description by the skill, then confirmed — never asked up-front.
- **Confirm proposed values before committing.** When the skill proposes a slug, a path, or a task structure, show it and ask for approval or edits.

## Interview flow

Follow these steps in order. Each step is a single message to the Operator.

- **Step 1 — Goal.** Ask the Operator to describe what they want to accomplish in their own words (one or two sentences). Do not ask about subtasks, repositories, or slugs yet.
- **Step 2 — Slug proposal.** From the goal, derive a short kebab-case slug (2–4 words, lowercase, hyphen-separated, e.g. `add-user-auth`). Propose it to the Operator and ask whether to use it or choose another.
- **Step 3 — Plan location base.** Ask the Operator which base directory to use:
  - **A.** Inside the Palette workflow's target repo (workspace) — plans ship with the code.
  - **B.** Inside the current CWD's repo — plans managed in an external repository (for example, managing Palette's own plans from a separate workspace repo).
- **Step 4 — Path confirmation.** Using the chosen base, construct the default directory:
  ```
  <base>/docs/plans/<YYYY>/<MMDD>-<slug>/
  ```
  - `<YYYY>` is the current year (e.g., `2026`).
  - `<MMDD>` is the current month and day (e.g., `0418`).
  - `<slug>` is the approved slug.
  Show the full path and ask the Operator to confirm or override.
- **Step 5 — Scope detail.** Ask the Operator for the scope and success criteria of the overall work (for the root plan's body). Keep it focused — one prompt, free-form answer.
- **Step 6 — Task breakdown.** Ask the Operator to describe the subtasks. For each subtask, the skill needs: key, type (`craft` or `review`), and any dependencies or target repository. If the Operator lists many subtasks at once, accept them; otherwise, ask about one at a time until they say the tree is complete.
- **Step 7 — Generation.** Generate both files in the chosen directory:
  - `blueprint.yaml` with the root task's `plan_path: README.md` set, so Palette's parser enforces the companion plan.
  - `README.md` with the goal, scope, success criteria, and a brief overview of the subtasks.
- **Step 8 — Review.** Show both generated files. Ask the Operator if any changes are needed. Apply requested edits in place.
- **Step 9 — Handoff.** Once the Operator approves, tell them to run `/palette:approve <absolute-path-to-blueprint.yaml>` to start the workflow.

## Blueprint YAML reference

Produce a structure like this:

```yaml
task:
  key: <root-slug>
  plan_path: README.md

  children:
    - key: <subtask-key>
      type: craft
      repository:
        name: <owner>/<repo>
        branch: <branch>
      # Optional: depends_on, priority, plan_path
    - key: <review-key>
      type: review
      depends_on: [<subtask-key>]
```

Rules to observe when generating the YAML:

- `task:` defines the root task with `key` (and optionally `plan_path`).
- Leaf tasks must have `type: craft` or `type: review`.
- Non-leaf (composite) tasks must NOT have a `type` field; they group child tasks via their own `children:` list.
- Use `depends_on:` to express ordering between sibling tasks.
- `priority:` can be `high`, `medium`, or `low`.
- `repository:` takes `name` and `branch` fields.
- `plan_path:` on any task (including the root) names a plan document **relative to the Blueprint directory**. Absolute paths and `..` are rejected by Palette.

## Notes

- Palette's Blueprint parser verifies every `plan_path` points to an existing file under the Blueprint directory. Skipping `README.md` when the root task declares it will cause workflow start to fail.
- A Blueprint that declares no `plan_path` on any task is also valid — this shape is used for purely mechanical workflows (such as auto-generated PR reviews). For hand-authored Blueprints, always include `plan_path: README.md` on the root so the intent is captured.
- In the future, a Crafter agent will generate Blueprints automatically. This skill serves as a manual bridge until that automation is ready.
