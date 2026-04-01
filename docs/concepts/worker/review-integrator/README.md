# Review Integrator

## Definition

The Review Integrator is a [Worker](../) that consolidates findings from multiple [Reviewers](../member/reviewer/) into a single verdict. It reads all `review.md` files produced by Reviewers, deduplicates findings, prioritizes them by severity, writes an `integrated-review.md`, and submits a unified review result. The Review Integrator is spawned only after all Reviewers have completed, so all inputs are available at startup.

The Review Integrator is **not** a [Supervisor](../supervisor/) — it does not receive permission prompts from Members. Permission prompts from Reviewers are handled by the [Permission Supervisor](../supervisor/permission-supervisor/).

## Examples

- The Review Integrator reads review reports from three Reviewers, removes duplicate findings, and submits a single "changes requested" verdict with a prioritized summary.
- The Review Integrator submits an "approved" verdict when no blocking issues are found across all Reviewers.

## Collocations

- consolidate (findings from multiple Reviewers)
- submit (a verdict)
- integrate (review results)

## Domain Rules

- The Review Integrator is spawned after all Reviewers have completed.
- All `review.md` files are available at startup — no waiting is required.
- The verdict is either "approved" or "changes requested."
- The Review Integrator does not decide whether to accept or reject review feedback — it only integrates and reports.

## Related Concepts

- [Worker](../) — the Review Integrator is a kind of Worker
- [Reviewer](../member/reviewer/) — produces the `review.md` files that the Review Integrator reads
- [Permission Supervisor](../supervisor/permission-supervisor/) — handles Reviewer permission prompts (the Review Integrator does not)
