# Review Integrator

## Definition

The Review Integrator is a [Supervisor](../) that consolidates findings from multiple [Reviewers](../../member/reviewer/) into a single verdict. The Review Integrator approves or denies permission prompts from Reviewers, deduplicates findings, prioritizes them by severity, and submits a unified review result. The Review Integrator does not judge whether review feedback should be accepted or rejected — that is the [Leader](../leader/)'s responsibility.

## Examples

- The Review Integrator receives review reports from three Reviewers, removes duplicate findings, and submits a single "changes requested" verdict with a prioritized summary.
- The Review Integrator approves a Reviewer's permission prompt to access a file needed for review.
- The Review Integrator submits an "approved" verdict when no blocking issues are found across all Reviewers.

## Collocations

- consolidate (findings from multiple Reviewers)
- submit (a verdict)
- approve (a permission prompt from a Reviewer)
- deny (a permission prompt from a Reviewer)

## Domain Rules

- The Review Integrator waits for all Reviewers to report before submitting a verdict.
- The verdict is either "approved" or "changes requested."
- The Review Integrator does not decide whether to accept or reject review feedback — it only integrates and reports.

## Related Concepts

- [Supervisor](../) — the Review Integrator is a kind of Supervisor
- [Reviewer](../../member/reviewer/) — the Review Integrator oversees Reviewers
- [Leader](../leader/) — evaluates the verdict submitted by the Review Integrator
