# Workflow

## Definition

A Workflow is an execution of a [Blueprint](../blueprint/). When the [Operator](../operator/) starts a Workflow from a Blueprint, Palette begins working through the [Task](../task/) tree defined in that Blueprint.

A Blueprint is a static definition of *what* should be done. A Workflow is the running instance — it tracks execution state such as Task status, [Job](../job/) assignments, review history, and timing.

## Examples

- The Operator starts a Workflow from a Blueprint for "add feature X." The Workflow tracks which Tasks are complete, which are in progress, and which are waiting.
- A Workflow is suspended when a [Leader](../worker/supervisor/leader/) raises an [Escalation](../escalation/). The Operator responds, and the Workflow resumes.
- A Workflow is complete when all Tasks in the Blueprint's Task tree are done.

## Collocations

- start (a Workflow from a Blueprint)
- suspend (a Workflow during an Escalation)
- resume (a Workflow after an Escalation is resolved)
- complete (a Workflow when all Tasks are done)

## Domain Rules

- A Workflow is started from exactly one Blueprint.
- A Workflow tracks execution state separately from the Blueprint.
- Multiple Workflows may run concurrently.

## Related Concepts

- [Blueprint](../blueprint/) — the static definition that a Workflow executes
- [Task](../task/) — the goals tracked within a Workflow
- [Job](../job/) — the work assignments tracked within a Workflow
- [Operator](../operator/) — starts a Workflow
- [Orchestrator](../orchestrator/) — manages Workflow execution
