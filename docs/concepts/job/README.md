# Job

## Definition

A Job is a work assignment on a [Task](../task/). A Task has at most one Job. A Job defines who works on the Task and how — the type of work (craft or review), the assigned [Member](../worker/member/), the repository and branch, and other execution details.

There are two kinds of Jobs:

- **Craft Job**: Assigned to a [Crafter](../worker/member/crafter/). Produces a deliverable such as code changes or a plan document.
- **Review Job**: Assigned to a [Reviewer](../worker/member/reviewer/). Inspects the deliverable of a Craft Job and reports findings to the [Review Integrator](../worker/supervisor/review-integrator/).

When a Review Job results in a "changes requested" verdict, the Crafter revises the work and a new round of review begins.

A Task has at most one Job. Dependencies between work are expressed through [Task](../task/) dependencies, not between Jobs.

## Examples

- A Craft Job: "Implement the dark mode toggle component."
- A Review Job: "Review the dark mode toggle implementation for correctness and test coverage."

## Collocations

- assign (a Job to a Member)
- complete (a Job)
- revise (a Craft Job after review feedback)

## Domain Rules

- A Task has at most one Job.
- A Job is assigned to exactly one Member.

## Related Concepts

- [Task](../task/) — the goal that this Job is assigned to
- [Member](../worker/member/) — the Worker that executes the Job
- [Crafter](../worker/member/crafter/) — executes Craft Jobs
- [Reviewer](../worker/member/reviewer/) — executes Review Jobs
- [Review Integrator](../worker/supervisor/review-integrator/) — consolidates findings from multiple Review Jobs
- [Plan](../plan/) — describes what the Job should accomplish
