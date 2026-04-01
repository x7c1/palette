# Approver

## Definition

The Approver is a [Supervisor](../) that approves or denies permission prompts from [Members](../../member/). It is assigned to a [Composite Task](../../../task/) and handles permission prompts from all child Members ([Crafters](../../member/crafter/) and [Reviewers](../../member/reviewer/)). The Approver has no other responsibilities — it does not evaluate review outcomes or write any files.

## Examples

- The Approver approves a Crafter's request to run a build command.
- The Approver denies a Reviewer's request that looks unrelated to the [Task](../../../task/).
- The Approver raises an [Escalation](../../../escalation/) when a decision could cause significant rework.

## Collocations

- approve (a permission prompt from a Member)
- deny (a permission prompt from a Member)
- escalate (a decision to the Operator)

## Related Concepts

- [Supervisor](../) — the Approver is a kind of Supervisor
- [Crafter](../../member/crafter/) — the Approver approves Crafter permissions
- [Reviewer](../../member/reviewer/) — the Approver approves Reviewer permissions
- [Escalation](../../../escalation/) — how the Approver reaches the Operator
- [Task](../../../task/) — the Approver is assigned to a Composite Task
