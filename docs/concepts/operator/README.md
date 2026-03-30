# Operator

## Definition

The Operator is the human who uses Palette. The Operator defines what should be achieved, starts a [Workflow](../workflow/) from a [Blueprint](../blueprint/), and receives the results. During execution, the Operator is absent — the system runs autonomously. The Operator returns when a [Supervisor](../worker/supervisor/) raises an [Escalation](../escalation/), or when the Operator chooses to suspend the Workflow to edit the Blueprint.

## Examples

- The Operator defines a [Task](../task/) such as "plan the next release of product A" or "add dark mode support to product A."
- The Operator reviews the final deliverables (branches, pull requests) after all [Jobs](../job/) are complete.
- The Operator responds to an Escalation when a Supervisor cannot make a judgment call on its own.
- The Operator suspends a running Workflow, edits the Blueprint to add new Tasks, applies the changes, and resumes the Workflow.

## Collocations

- start (a Workflow from a Blueprint)
- suspend (a Workflow)
- resume (a Workflow)
- edit (a Blueprint during suspend)
- apply (a Blueprint change)
- respond (to an Escalation)
- review (the final deliverables)

## Related Concepts

- [Task](../task/) — what the Operator wants to achieve
- [Workflow](../workflow/) — the execution that the Operator starts
- [Blueprint](../blueprint/) — the plan that defines the Workflow
- [Escalation](../escalation/) — how the system reaches the Operator during execution
- [Supervisor](../worker/supervisor/) — the agent that acts on behalf of the Operator
