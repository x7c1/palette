# Supervisor

## Definition

A Supervisor is a [Worker](../) that oversees [Members](../member/) on behalf of the [Operator](../../operator/). Supervisors make the runtime decisions that the Operator would otherwise need to make — approving actions, evaluating results, and raising [Escalations](../../escalation/) when a decision exceeds their confidence.

There are two kinds of Supervisors:

- [Leader](leader/) — oversees [Crafters](../member/crafter/) and coordinates the overall [Task](../../task/).
- [Review Integrator](review-integrator/) — consolidates findings from multiple [Reviewers](../member/reviewer/) into a single verdict.

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
- [Leader](leader/) — a kind of Supervisor
- [Review Integrator](review-integrator/) — a kind of Supervisor
