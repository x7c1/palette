---
name: plan
description: Interview the Operator about a new task and generate the Blueprint and plan documents
user-invocable: true
---

# /palette:plan

Interview the Operator about a new task, then generate a `blueprint.yaml` and its companion `README.md`. The plan is referenced by the Blueprint's root task via `plan_path` and lives alongside `blueprint.yaml` in the same directory.

## Instructions

- Interview the Operator to learn the goal, target repository, and scope
- Investigate the target repository and share a short summary so the Operator can correct any misreadings
- Propose a task breakdown grounded in the investigation; confirm with the Operator
- Propose the slug and the plan directory path; confirm with the Operator
- Generate `blueprint.yaml` and `README.md`
- Review the generated files with the Operator, apply edits, then hand off to `/palette:approve`

## Interaction Principles

- Ask one question at a time and wait for the answer before moving on
- Keep each message to one or two sentences — do not rehearse later steps, explain internal stages, or mention `slug`/filesystem details to the Operator
- Confirm proposed values (slug, path, task tree) before committing them
- Hold off proposing the slug until scope and breakdown are known; the Operator's own domain vocabulary should shape it, not their opening phrasing

## Interview Questions

Ask these in order. One question per message.

- What would you like to accomplish?
- Which repository should this target? (expect an `owner/repo` answer; do not ask about branches)
- What's the scope, and what would mark this as done?

If the Operator volunteers that multiple repositories are involved, record it and use it later during the breakdown. Otherwise assume a single target repository.

## Repository Investigation

Do this alone — do not ask the Operator questions during investigation.

- Read the repository's entry points (top-level README, relevant docs, the source directories the scope points at)
- Use Grep / Glob to locate affected modules, existing patterns, tests, and related specs
- Identify: the files in scope for modification, the conventions the change should match, and whether the work is one concern or several independent concerns
- Record a short **vocabulary list** — the canonical names the repo uses for the concepts, types, modules, and subsystems you touched. This list will anchor the wording of the generated plan so it mirrors what the codebase already calls things

Conclude the step by posting a short summary to the Operator — "Here's what I found: …" — listing the affected files and the key observations that will drive the breakdown. Invite corrections.

## Task Breakdown Policy

Propose the task tree yourself; do not ask the Operator to enumerate subtasks.

- If the work is a single small implementation concern, emit **one** craft task (with its implicit review child). Do not invent artificial splits.
- If the work has multiple distinct implementation concerns, sequential phases, or sub-steps that can fail or be reviewed independently, emit **multiple** craft tasks with `depends_on` for ordering.
- When uncertain, prefer a single task — splitting later is cheap, merging is not.

Derive every technical field yourself:

- `key`: a kebab-case summary of each task (2–4 words), drawn from the vocabulary that surfaced during scope and investigation
- `type`: always `craft` for concrete work items; each one implicitly owns a `review` child
- `depends_on`: inferred from the scope's sequencing
- `repository`: reuse the target repository
- `work_branch`: the branch Palette will commit to. Propose `feature/<craft-key>` and offer it to the Operator for override. The orchestrator creates this branch (it does not need to exist on the remote yet); see `source_branch` below
- `source_branch`: omit by default so Palette derives the work branch from the repository's default branch. Set it only when the Operator explicitly asks to derive from a non-default branch
- `priority`: leave unset unless the Operator explicitly flagged priorities

Present the proposed tree as a rendered YAML snippet with a one-line rationale ("single task — scope is self-contained" / "split into N tasks because …") and ask the Operator to confirm or request changes. Only ask follow-ups when a field genuinely cannot be derived (e.g. multi-repo ambiguity).

## Slug and Path

- Slug: kebab-case, 2–4 words, lowercase, drawn from vocabulary that surfaced during scope and breakdown (often a subtask key or a key noun from the scope)
- Propose the slug with a brief rationale; the Operator may accept or override
- Default plan base: the target repository itself (plans ship with the code)
- Alternative base: the current CWD's repo (for plans managed in an external repository — e.g. a private workspace repo hosting another project's plans)
- Default directory layout:
  ```
  <base>/docs/plans/<YYYY>/<MMDD>-<slug>/
  ```
  where `<YYYY>` is the current year and `<MMDD>` is the current month and day
- Show the full path and ask the Operator to confirm or override

## Generation

Write both files in the chosen directory:

- `blueprint.yaml` — the task tree, with the root task's `plan_path: README.md` set so Palette's parser enforces the companion plan
- `README.md` — the plan document: goal, scope, success criteria, brief overview of the subtasks

Before showing the files to the Operator, perform a **vocabulary check** against the list captured during Repository Investigation:

- For each concept mentioned in `README.md`, verify it uses the repo's existing name when one exists. If a draft sentence paraphrases an established concept (e.g. invents "task definition" when the codebase says `Blueprint`, or writes "executor" for a `Crafter`), replace the paraphrase with the canonical term
- Prefer the canonical name even when it is slightly longer or less conversational — consistency with the codebase outweighs stylistic variation
- If the plan introduces a genuinely new concept that has no existing name in the repo, keep your chosen wording, but call it out explicitly in the plan so the novelty is visible

After writing both files, run a pre-flight check against Palette to catch structural mistakes before handoff:

```
POST /blueprints/validate
Content-Type: application/json

{ "blueprint_path": "<absolute path to the generated blueprint.yaml>" }
```

- **200 with `valid: true`** — summary shows the task/craft/review counts. Proceed to review with the Operator.
- **200 with `valid: false`** — fix each item in `errors[]` (each carries a `hint` pointing at the offending field and a machine-readable `reason` like `blueprint/missing_review_child` or `blueprint/plan_path_missing`), then re-validate. Do not hand off while any errors remain.
- **404** — the path is wrong; regenerate the file or correct the path before re-validating.

Show both files to the Operator and apply any requested edits in place. Once approved, tell the Operator to run `/palette:approve` to start the workflow — no path argument needed, since `/palette:approve` picks up the blueprint just generated from the conversation context.

## Blueprint YAML Reference

```yaml
task:
  key: <root-slug>
  plan_path: README.md

  children:
    - key: <craft-key>
      type: craft
      repository:
        name: <owner>/<repo>
        work_branch: feature/<craft-key>
        # Optional: source_branch: <branch>   # omit → repository default branch
      # Optional: depends_on, priority, plan_path
      children:
        - key: <review-key>
          type: review
```

Rules:

- `task:` defines the root task with `key` (and optionally `plan_path`)
- Leaf tasks must have `type: craft` or `type: review`
- **Every `craft` task must carry a `review` child** — Palette rejects a Blueprint with a `craft` task that has no review child. The review's ordering relative to the craft is implicit, so do not add `depends_on:` for it
- Non-leaf (composite) tasks must NOT have a `type` field; they group child tasks via their own `children:` list
- Use `depends_on:` to express ordering between **sibling** tasks (e.g. a later craft that depends on an earlier craft finishing)
- `priority:` can be `high`, `medium`, or `low`
- `repository:` takes `name`, `work_branch`, and an optional `source_branch` field. `work_branch` is the branch Palette creates and commits to; `source_branch` names the branch to derive it from (omit to fall back to the repository's default branch)
- `plan_path:` on any task names a plan document **relative to the Blueprint directory**. Absolute paths and `..` are rejected

## Notes

- Palette's Blueprint parser verifies every `plan_path` points to an existing file under the Blueprint directory. Skipping `README.md` when the root task declares it will cause workflow start to fail.
- A Blueprint that declares no `plan_path` on any task is also valid — this shape is used for purely mechanical workflows (such as auto-generated PR reviews). For hand-authored Blueprints, always include `plan_path: README.md` on the root so the intent is captured.
- In the future, a Crafter agent will generate Blueprints automatically. This skill serves as a manual bridge until that automation is ready.
