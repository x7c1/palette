# Operator

## Definition

The Operator is the human who uses Palette. The Operator defines what should be achieved, starts a [Workflow](../workflow/) from a [Blueprint](../blueprint/), and receives the results. During execution, the Operator is absent — the system runs autonomously. The Operator only returns when a [Supervisor](../worker/supervisor/) raises an [Escalation](../escalation/).

## Examples

- The Operator gives Palette a [Task](../task/) such as "plan the next release of product A" or "add dark mode support to product A."
- The Operator reviews the final deliverables (branches, pull requests) after all [Jobs](../job/) are complete.
- The Operator responds to an Escalation when a Supervisor cannot make a judgment call on its own.

## Collocations

- start (a Workflow from a Blueprint)
- respond (to an Escalation)
- review (the final deliverables)

## Related Concepts

- [Task](../task/) — what the Operator wants to achieve
- [Workflow](../workflow/) — the execution that the Operator starts
- [Blueprint](../blueprint/) — the plan that defines the Workflow
- [Escalation](../escalation/) — how the system reaches the Operator during execution
- [Supervisor](../worker/supervisor/) — the agent that acts on behalf of the Operator
