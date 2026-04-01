# Supervisor

## Definition

A Supervisor is a [Worker](../) that oversees [Members](../member/) on behalf of the [Operator](../../operator/). Supervisors make the runtime decisions that the Operator would otherwise need to make — approving permission prompts and raising [Escalations](../../escalation/) when a decision exceeds their confidence.

[Approver](approver/) is the only Supervisor type. The [Review Integrator](../../review-integrator/) is not a Supervisor — it is a standalone Worker that reads review files and submits verdicts without receiving member events.

## Examples

- A Supervisor approves a permission prompt from a Member, allowing the Member to continue its [Job](../../job/).
- A Supervisor raises an Escalation to the Operator when it cannot make a judgment call on its own.

## Collocations

- oversee (Members)
- approve (a permission prompt)
- deny (a permission prompt)
- escalate (a decision to the Operator)

## Related Concepts

- [Operator](../../operator/) — the Supervisor acts on behalf of the Operator
- [Member](../member/) — the Worker that a Supervisor oversees
- [Escalation](../../escalation/) — how the Supervisor reaches the Operator
- [Approver](approver/) — the only kind of Supervisor
