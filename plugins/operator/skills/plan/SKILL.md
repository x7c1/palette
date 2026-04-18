---
name: plan
description: Interview the Operator about a new task and generate the Blueprint and plan documents
user-invocable: true
---

# /palette:plan

Interview the Operator about a new task, then generate a Blueprint YAML and its companion plan (`README.md`). The plan is referenced by the Blueprint's root task via `plan_path` and lives alongside `blueprint.yaml` in the same directory.

## Interaction principles

- **Ask one question at a time.** Wait for the Operator's answer before moving on. Do not batch multiple questions in a single message.
- **Defer naming until the work is fully described.** The slug is the directory name, so it is filesystem-facing but not needed until generation. Hold off proposing a slug until scope and subtasks are known — by then the Operator's own subtask names reveal the domain vocabulary the skill should use, and any imprecise wording from the opening goal has had a chance to settle.
- **Confirm proposed values before committing.** When the skill proposes a slug, a path, or a task structure, show it and ask for approval or edits.

## Interview flow

Follow these steps in order. Each step is a single message to the Operator.

- **Step 1 — Goal.** Ask the Operator to describe what they want to accomplish in their own words (one or two sentences). Tell them you will settle on a name at the end — for now, just the goal.
- **Step 2 — Target repository.** Ask which repository this work targets (just the `owner/repo` name — not the branch). Assume a single repo by default; if the Operator says the work spans multiple repositories, accept that and note it for Step 4. Branches are per-craft-task and are decided during Step 4, so do not ask about branches here.
- **Step 3 — Scope detail.** Ask the Operator for the scope and success criteria of the overall work (for the root plan's body). Keep it focused — one prompt, free-form answer.
- **Step 4 — Task breakdown.** Ask the Operator **only** for a natural-language description of the subtasks ("どういう作業のかたまりに分けますか?" / "What pieces of work should this break into?"). Accept the answer as free-form prose or a short list. Derive every technical field yourself from that answer:
  - `key`: a kebab-case summary of each subtask (2–4 words).
  - `type`: default `craft` for every concrete work item. Every craft task is given a `review` child automatically — do not ask the Operator about reviews; they are implied by the schema.
  - `depends_on`: inferred from ordering words in the Operator's answer ("まず…次に…", "first… then…"). Tasks described as sequential become `depends_on: [<previous>]`; tasks presented as a flat list stay independent.
  - `repository` / `branch`: reuse Step 2's repository and default its branch to the repository's default branch (inspect the repo if possible). Do not prompt for these.
  - `priority`: leave unset unless the Operator explicitly flags a subtask as high/medium/low priority in their description.

  Ask a follow-up **only** when a field genuinely cannot be derived:
  - The Operator declared a multi-repo workflow in Step 2 and it is ambiguous which repo owns a given subtask.
  - The Operator mentioned a specific non-default branch but did not attach it to a subtask.

  After deriving the tree, present it back as a rendered YAML snippet and ask the Operator to confirm or amend ("この分解でよいですか? 修正があれば教えてください"). Apply any requested edits in place before moving on.
- **Step 5 — Slug proposal.** Now that the goal, repository, scope, and subtasks are all known, derive a short kebab-case slug (2–4 words, lowercase, hyphen-separated, e.g. `refresh-keybinding`). Base it on the vocabulary that surfaced during Step 3 and Step 4 — often a subtask key, a key noun from the scope, or a concatenation thereof — rather than on the Operator's opening phrasing. Propose it with a brief rationale and ask the Operator to accept or override.
- **Step 6 — Plan location base.** Ask which base directory to use:
  - **A.** Inside the target repository itself — plans ship with the code (the common case).
  - **B.** Inside the current CWD's repo — plans managed in an external repository (for example, managing a private project's plans from a separate workspace repo).
  Present **A** first with the concrete repo name filled in so the choice is unambiguous.
- **Step 7 — Path confirmation.** Using the chosen base, construct the default directory:
  ```
  <base>/docs/plans/<YYYY>/<MMDD>-<slug>/
  ```
  - `<YYYY>` is the current year (e.g., `2026`).
  - `<MMDD>` is the current month and day (e.g., `0418`).
  - `<slug>` is the approved slug.
  Show the full path and ask the Operator to confirm or override.
- **Step 8 — Generation.** Generate both files in the chosen directory:
  - `blueprint.yaml` with the root task's `plan_path: README.md` set, so Palette's parser enforces the companion plan.
  - `README.md` with the goal, scope, success criteria, and a brief overview of the subtasks.
- **Step 9 — Review.** Show both generated files. Ask the Operator if any changes are needed. Apply requested edits in place.
- **Step 10 — Handoff.** Once the Operator approves, tell them to run `/palette:approve <absolute-path-to-blueprint.yaml>` to start the workflow.

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
