# Escalation

## Definition

An Escalation occurs when the system requires the [Operator](../operator/)'s judgment to proceed. During an Escalation, the affected [Task](../task/) is suspended until the Operator responds.

An Escalation can be triggered in two ways:

- **Automatic**: The [Orchestrator](../orchestrator/) raises an Escalation when a predefined rule is violated — for example, when the review cycle exceeds the maximum number of rounds.
- **Voluntary**: A [Supervisor](../worker/supervisor/) raises an Escalation when it encounters a decision beyond its confidence — for example, when a [Permission Supervisor](../worker/supervisor/permission-supervisor/) cannot determine whether a permission request is safe to approve.

## Examples

- The review cycle for a Craft Job has gone through three rounds without approval. The Orchestrator raises an Escalation to notify the Operator.
- The Permission Supervisor is unsure whether a permission request from a [Crafter](../worker/member/crafter/) is safe. The Permission Supervisor raises an Escalation rather than guessing.
- The Operator responds to an Escalation, and the suspended Task resumes.

## Collocations

- raise (an Escalation)
- respond (to an Escalation)
- suspend (a Task during an Escalation)
- resume (a Task after an Escalation is resolved)

## Domain Rules

- An Escalation suspends the affected Task until the Operator responds.
- Other Tasks that are not affected may continue to run during an Escalation.

## Related Concepts

- [Operator](../operator/) — the recipient of an Escalation
- [Supervisor](../worker/supervisor/) — may raise a voluntary Escalation
- [Orchestrator](../orchestrator/) — may raise an automatic Escalation
- [Task](../task/) — suspended during an Escalation
