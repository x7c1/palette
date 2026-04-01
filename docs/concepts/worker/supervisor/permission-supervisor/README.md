# Permission Supervisor

## Definition

The Permission Supervisor is a [Supervisor](../) that approves or denies permission prompts from [Members](../../member/). It is assigned to a [Composite Task](../../../task/) and handles permission prompts from all child Members ([Crafters](../../member/crafter/) and [Reviewers](../../member/reviewer/)). The Permission Supervisor has no other responsibilities — it does not evaluate review outcomes or write any files.

## Examples

- The Permission Supervisor approves a Crafter's request to run a build command.
- The Permission Supervisor denies a Reviewer's request that looks unrelated to the [Task](../../../task/).
- The Permission Supervisor raises an [Escalation](../../../escalation/) when a decision could cause significant rework.

## Collocations

- approve (a permission prompt from a Member)
- deny (a permission prompt from a Member)
- escalate (a decision to the Operator)

## Related Concepts

- [Supervisor](../) — the Permission Supervisor is a kind of Supervisor
- [Crafter](../../member/crafter/) — the Permission Supervisor approves Crafter permissions
- [Reviewer](../../member/reviewer/) — the Permission Supervisor approves Reviewer permissions
- [Escalation](../../../escalation/) — how the Permission Supervisor reaches the Operator
- [Task](../../../task/) — the Permission Supervisor is assigned to a Composite Task
