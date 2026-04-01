# Crafter

## Definition

A Crafter is a [Member](../) that executes a Craft [Job](../../../job/). The Crafter produces deliverables — such as code changes, tests, or a [Blueprint](../../../blueprint/) (during the planning phase) — and is overseen by the [Permission Supervisor](../../supervisor/permission-supervisor/). When the Crafter needs to perform a potentially risky action, it sends a permission prompt to the Permission Supervisor for approval.

After completing a Craft Job, the Crafter's deliverables are reviewed by [Reviewers](../reviewer/). If the review results in a "changes requested" verdict, the Crafter revises the work based on the feedback.

## Examples

- A Crafter implements a new feature and sends a permission prompt to the Permission Supervisor before running a destructive command.
- A Crafter receives review feedback and revises the work accordingly.
- A Crafter produces a Blueprint during the planning phase — defining the Job breakdown and creating Plans for the Task and each Job.

## Collocations

- craft (a deliverable)
- revise (work based on review feedback)
- request (permission from the Permission Supervisor)

## Related Concepts

- [Member](../) — the Crafter is a kind of Member
- [Permission Supervisor](../../supervisor/permission-supervisor/) — the Supervisor that oversees the Crafter
- [Reviewer](../reviewer/) — inspects the Crafter's deliverables
- [Job](../../../job/) — the Crafter executes Craft Jobs
