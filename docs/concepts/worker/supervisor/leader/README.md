# Leader

## Definition

The Leader is a [Supervisor](../) that is assigned to a [Composite Task](../../../task/) to supervise its child Tasks. The Leader approves or denies permission prompts from [Crafters](../../member/crafter/), evaluates review outcomes, and resolves conflicts between implementation decisions and review feedback. When a decision exceeds the Leader's confidence, the Leader raises an [Escalation](../../../escalation/) to the [Operator](../../../operator/), suspending the affected Task until the Operator responds.

## Examples

- The Leader approves a Crafter's request to run a build command.
- The Leader denies a Crafter's request that looks unrelated to the [Task](../../../task/).
- The Leader receives a "changes requested" review verdict and determines whether the feedback aligns with the Task's goals.
- The Leader raises an Escalation when a decision could cause significant rework.

## Collocations

- approve (a permission prompt from a Crafter)
- deny (a permission prompt from a Crafter)
- evaluate (a review outcome)
- escalate (a decision to the Operator)

## Related Concepts

- [Supervisor](../) — the Leader is a kind of Supervisor
- [Crafter](../../member/crafter/) — the Leader oversees Crafters
- [Escalation](../../../escalation/) — how the Leader reaches the Operator
- [Review Integrator](../review-integrator/) — the other kind of Supervisor; submits review verdicts that the Leader evaluates
- [Task](../../../task/) — the Leader supervises a Composite Task
