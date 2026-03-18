# Reviewer

## Definition

A Reviewer is a [Member](../) that executes a Review [Job](../../../job/). The Reviewer inspects the deliverables produced by a [Crafter](../crafter/) and reports findings to the [Review Integrator](../../supervisor/review-integrator/). Multiple Reviewers may work in parallel on the same Craft Job, each examining the deliverables independently.

The [Review Integrator](../../supervisor/review-integrator/) consolidates findings from all Reviewers into a single verdict.

## Examples

- A Reviewer examines a code change for correctness, security issues, and test coverage, then reports its findings to the Review Integrator.
- A Reviewer evaluates a Blueprint during the planning phase — checking whether the Job breakdown is appropriate and whether the Plans are consistent and feasible.

## Collocations

- inspect (a Crafter's deliverables)
- report (findings to the Review Integrator)

## Related Concepts

- [Member](../) — the Reviewer is a kind of Member
- [Review Integrator](../../supervisor/review-integrator/) — the Supervisor that oversees the Reviewer
- [Crafter](../crafter/) — produces the deliverables that the Reviewer inspects
- [Job](../../../job/) — the Reviewer executes Review Jobs
