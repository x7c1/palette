---
name: plan
description: Interview the Operator about a new task and generate the Blueprint and plan documents
user-invocable: true
---

# /palette:plan

Interview the Operator about a new task, then generate a Blueprint YAML and its companion plan (`README.md`). The plan is referenced by the Blueprint's root task via `plan_path` and lives alongside `blueprint.yaml` in the same directory.

## Interaction principles

- **Ask one question at a time.** Wait for the Operator's answer before moving on. Do not batch multiple questions in a single message.
- **Keep messages short and direct.** A message to the Operator is typically one sentence — at most two. Do not rehearse what comes next, explain filesystem details, or announce internal stages like "slug will come later." Those are instructions to you, not to the Operator.
- **Defer naming until the work is fully described.** The slug is the directory name, so it is filesystem-facing but not needed until generation. Hold off proposing a slug until scope and subtasks are known — by then the Operator's own subtask names reveal the domain vocabulary the skill should use, and any imprecise wording from the opening goal has had a chance to settle.
- **Confirm proposed values before committing.** When the skill proposes a slug, a path, or a task structure, show it and ask for approval or edits.

## Interview flow

Follow these steps in order. Each step is a single message to the Operator.

- **Step 1 — Goal.** Ask the Operator what they want to accomplish. Example phrasing: "What would you like to accomplish?" Do not mention slugs, subtasks, or later steps — just the question.
- **Step 2 — Target repository.** Ask which repository this work targets. Example: "Which repository should this target?" Expect an `owner/repo` answer. Do not ask about branches here; branches belong to craft tasks and are resolved in Step 5. If the Operator volunteers that multiple repositories are involved, record that for Step 5.
- **Step 3 — Scope detail.** Ask for the scope and success criteria in one focused question. Example: "What's the scope and what would mark this as done?" Accept a free-form answer.
- **Step 4 — Investigation (skill works alone).** Before proposing a task breakdown, actively inspect the target repository to ground your understanding. Do not ask the Operator any questions during this step.
  - Read the repository's entry points (top-level README, relevant docs, relevant source directories) and files the stated scope points at.
  - Use Grep/Glob to locate affected modules, existing patterns, naming conventions, tests, and related specs.
  - Identify: which files are in scope for modification, what existing patterns the change should match, any design constraints, and whether the work is one concern or several independent concerns.
  - Finish the step by posting a short summary to the Operator: "Here's what I found: …" listing the affected files/modules and the key observations that will drive the breakdown. Ask the Operator to flag anything you misread before you propose a task tree. This is the Operator's chance to correct your understanding cheaply.
- **Step 5 — Task breakdown (skill proposes, Operator confirms).** Using the investigation from Step 4, propose the task tree. Do **not** ask the Operator to enumerate subtasks.
  - If the work is a single small implementation concern, emit **one** craft task (with its implicit review child). Do not invent artificial splits.
  - If the work has multiple distinct implementation concerns, sequential phases, or sub-steps that can fail/be reviewed independently, emit **multiple** craft tasks, using `depends_on` for any required ordering.
  - When uncertain whether to split, prefer a single task — splitting later is cheap, merging is not.

  Derive every technical field yourself:
  - `key`: a kebab-case summary of each task (2–4 words), drawn from the vocabulary that surfaced in Step 3 and Step 4.
  - `type`: always `craft` for concrete work items; each one implicitly owns a `review` child.
  - `depends_on`: inferred from the scope's sequencing.
  - `repository` / `branch`: reuse Step 2's repository; default the branch to the repository's default branch (inspect the repo if possible).
  - `priority`: leave unset unless the Operator flagged specific priorities in Step 3.

  Present the proposed tree as a rendered YAML snippet with a one-line rationale ("single task — scope is self-contained" / "split into N tasks because …") and ask the Operator to confirm the breakdown or request changes (add, remove, reorder). Only ask a follow-up when a field genuinely cannot be derived (e.g. multi-repo ambiguity from Step 2). Apply requested edits in place before moving on.
- **Step 6 — Slug proposal.** Now that the goal, repository, scope, investigation, and subtasks are all known, derive a short kebab-case slug (2–4 words, lowercase, hyphen-separated, e.g. `refresh-keybinding`). Base it on the vocabulary that surfaced during Steps 3–5 — often a subtask key, a key noun from the scope, or a concatenation thereof — rather than on the Operator's opening phrasing. Propose it with a brief rationale and ask the Operator to accept or override.
- **Step 7 — Plan location base.** Ask which base directory to use:
  - **A.** Inside the target repository itself — plans ship with the code (the common case).
  - **B.** Inside the current CWD's repo — plans managed in an external repository (for example, managing a private project's plans from a separate workspace repo).
  Present **A** first with the concrete repo name filled in so the choice is unambiguous.
- **Step 8 — Path confirmation.** Using the chosen base, construct the default directory:
  ```
  <base>/docs/plans/<YYYY>/<MMDD>-<slug>/
  ```
  - `<YYYY>` is the current year (e.g., `2026`).
  - `<MMDD>` is the current month and day (e.g., `0418`).
  - `<slug>` is the approved slug.
  Show the full path and ask the Operator to confirm or override.
- **Step 9 — Generation.** Generate both files in the chosen directory:
  - `blueprint.yaml` with the root task's `plan_path: README.md` set, so Palette's parser enforces the companion plan.
  - `README.md` with the goal, scope, success criteria, and a brief overview of the subtasks.
- **Step 10 — Review.** Show both generated files. Ask the Operator if any changes are needed. Apply requested edits in place.
- **Step 11 — Handoff.** Once the Operator approves, tell them to run `/palette:approve <absolute-path-to-blueprint.yaml>` to start the workflow.

## Blueprint YAML reference

Produce a structure like this:

```yaml
task:
  key: <root-slug>
  plan_path: README.md

  children:
    - key: <craft-key>
      type: craft
      repository:
        name: <owner>/<repo>
        branch: <branch>
      # Optional: depends_on, priority, plan_path
      children:
        - key: <review-key>
          type: review
```

Rules to observe when generating the YAML:

- `task:` defines the root task with `key` (and optionally `plan_path`).
- Leaf tasks must have `type: craft` or `type: review`.
- **Every `craft` task must carry a `review` child** — Palette rejects a Blueprint with a `craft` task that has no review child. The review's ordering relative to the craft is implied by the parent-child relationship, so do not add `depends_on:` for it.
- Non-leaf (composite) tasks must NOT have a `type` field; they group child tasks via their own `children:` list.
- Use `depends_on:` to express ordering between **sibling** tasks (e.g. a later craft that depends on an earlier craft finishing).
- `priority:` can be `high`, `medium`, or `low`.
- `repository:` takes `name` and `branch` fields.
- `plan_path:` on any task (including the root) names a plan document **relative to the Blueprint directory**. Absolute paths and `..` are rejected by Palette.

## Notes

- Palette's Blueprint parser verifies every `plan_path` points to an existing file under the Blueprint directory. Skipping `README.md` when the root task declares it will cause workflow start to fail.
- A Blueprint that declares no `plan_path` on any task is also valid — this shape is used for purely mechanical workflows (such as auto-generated PR reviews). For hand-authored Blueprints, always include `plan_path: README.md` on the root so the intent is captured.
- In the future, a Crafter agent will generate Blueprints automatically. This skill serves as a manual bridge until that automation is ready.
