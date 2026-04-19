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

- `active` — running.
- `suspending` — suspend requested; in-progress work is winding down and new work is blocked.
- `suspended` — paused; the Workflow can be resumed.
- `completed` — terminal. All Tasks in the Blueprint's Task tree are done.
- `terminated` — terminal. The Orchestrator's shutdown stopped the Workflow; it cannot be resumed.
- `failed` — terminal. A runtime failure stopped the Workflow before it could complete.

`terminated` and `failed` are both terminal but are kept distinct: `terminated` is an operator-driven outcome and is not a fault signal, whereas `failed` indicates the Workflow itself could not proceed.

A failed Workflow carries a **failure reason** that names the cause in a machine-readable form (e.g. "workspace setup failed," "branch in use"). The reason is the only failure metadata the Workflow itself retains; detailed diagnostics belong in operational logs rather than on the Workflow.

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
