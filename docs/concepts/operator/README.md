# Operator

## Definition

The Operator is the human who uses Palette. The Operator defines what should be achieved, starts the system, and receives the results. During execution, the Operator is absent — the system runs autonomously. The Operator only returns when a [Supervisor](../worker/supervisor/) raises an [Escalation](../escalation/).

## Examples

- The Operator gives Palette a [Task](../task/) such as "plan the next release of product A" or "add dark mode support to product A."
- The Operator reviews the final deliverables (branches, pull requests) after all [Jobs](../job/) are complete.
- The Operator responds to an Escalation when a Supervisor cannot make a judgment call on its own.

## Collocations

- start (the system with a Task)
- respond (to an Escalation)
- review (the final deliverables)

## Related Concepts

- [Task](../task/) — what the Operator wants to achieve
- [Escalation](../escalation/) — how the system reaches the Operator during execution
- [Supervisor](../worker/supervisor/) — the agent that acts on behalf of the Operator
