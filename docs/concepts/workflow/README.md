# Workflow

## Definition

A Workflow is an execution of a [Blueprint](../blueprint/). When the [Operator](../operator/) starts a Workflow from a Blueprint, Palette begins working through the [Task](../task/) tree defined in that Blueprint.

A Blueprint is a static definition of *what* should be done. A Workflow is the running instance — it tracks execution state such as Task status, [Job](../job/) assignments, review history, and timing.

## Examples

- The Operator starts a Workflow from a Blueprint for "add feature X." The Workflow tracks which Tasks are complete, which are in progress, and which are waiting.
- A Workflow is suspended when an [Approver](../worker/supervisor/approver/) raises an [Escalation](../escalation/). The Operator responds, and the Workflow resumes.
- The Operator suspends a Workflow to edit the [Blueprint](../blueprint/), applies the changes, and resumes the Workflow. New [Tasks](../task/) added to the Blueprint are picked up on resume.
- A Workflow is complete when all Tasks in the Blueprint's Task tree are done.

## Collocations

- start (a Workflow from a Blueprint)
- suspend (a Workflow — by the Operator or due to an Escalation)
- resume (a Workflow after suspend)
- complete (a Workflow when all Tasks are done)
- fail (a Workflow when a runtime error prevents it from continuing)

## Status

A Workflow carries one of the following statuses:

- `active` — running, assignments and messages flow normally.
- `suspending` — suspend requested; in-progress workers finish while new work is blocked.
- `suspended` — all workers stopped; the Workflow can be resumed.
- `completed` — terminal. All Tasks in the Blueprint's Task tree are done.
- `terminated` — terminal. The Orchestrator's explicit shutdown stopped the Workflow; workers are destroyed and the Workflow cannot be resumed.
- `failed` — terminal. A runtime failure (e.g. workspace setup failed, branch conflict) stopped the Workflow. Carries a `failure_reason` key that identifies the cause.

`terminated` and `failed` are both terminal but are kept distinct: `terminated` is caused by operator-driven shutdown and is not a fault signal, whereas `failed` indicates the Workflow itself could not proceed. Failed Workflows are reclaimed through `palette admin gc` or `palette admin reset`; individual resume/retry APIs are intentionally out of scope at this point.

### failure_reason

When a Workflow transitions to `failed`, the row also stores a `failure_reason` string. Reasons are machine-readable keys in `{namespace}/{value}` form and use the `workflow/` namespace — for example `workflow/workspace_setup_failed`, `workflow/branch_in_use`, `workflow/git_push_failed`.

Callers must pass a key produced by a `#[derive(palette_macros::ReasonKey)]` enum's `.reason_key()` method rather than a literal string. This keeps reason names typo-resistant and makes it easy to grep for every place a given reason can originate. Detailed error context (stderr, stack traces) stays in `tracing::error!` logs and is deliberately not persisted to the DB; the reason key is the only failure metadata stored on the Workflow row.

`mark_workflow_failed` is a no-op on Workflows already in a terminal state (`completed`, `terminated`, or `failed`), so a late failure notification cannot overwrite a previously recorded terminal outcome.

## Domain Rules

- A Workflow is started from exactly one Blueprint.
- A Workflow tracks execution state separately from the Blueprint.
- Multiple Workflows may run concurrently.
- Terminal states (`completed`, `terminated`, `failed`) are one-way; a Workflow never leaves them.

## Related Concepts

- [Blueprint](../blueprint/) — the static definition that a Workflow executes
- [Task](../task/) — the goals tracked within a Workflow
- [Job](../job/) — the work assignments tracked within a Workflow
- [Operator](../operator/) — starts a Workflow
- [Orchestrator](../orchestrator/) — manages Workflow execution
