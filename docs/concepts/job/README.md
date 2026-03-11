# Job

## Definition

A Job is a unit of work assigned to a single [Member](../worker/member/). Each Job belongs to a [Task](../task/) and represents one concrete step toward completing that Task.

There are two kinds of Jobs:

- **Craft Job**: Assigned to a [Crafter](../worker/member/crafter/). Produces a deliverable such as code changes or a plan.
- **Review Job**: Assigned to a [Reviewer](../worker/member/reviewer/). Inspects the deliverable of a Craft Job and reports findings to the [Review Integrator](../worker/supervisor/review-integrator/).

A Task may involve multiple rounds of Craft Jobs and Review Jobs. When a Review Job results in a "changes requested" verdict, the Crafter revises the work and a new round of review begins.

## Examples

- A Craft Job: "Implement the dark mode toggle component."
- A Review Job: "Review the dark mode toggle implementation for correctness and test coverage."

## Collocations

- assign (a Job to a Member)
- complete (a Job)
- revise (a Craft Job after review feedback)

## Domain Rules

- A Job is assigned to exactly one Member.
- A Review Job can only begin after the associated Craft Job enters review.
- Multiple Review Jobs may run in parallel for the same Craft Job.

## Related Concepts

- [Task](../task/) — the goal that this Job contributes to
- [Member](../worker/member/) — the Worker that executes the Job
- [Crafter](../worker/member/crafter/) — executes Craft Jobs
- [Reviewer](../worker/member/reviewer/) — executes Review Jobs
- [Review Integrator](../worker/supervisor/review-integrator/) — consolidates findings from multiple Review Jobs
