# Task

## Definition

A Task is a goal that the [Operator](../operator/) wants to achieve. It describes *what* should be accomplished, not *how* to accomplish it.

A Task can be broken down into child Tasks or into [Jobs](../job/). A planning Task produces child Tasks as its outcome; an implementation Task is broken down into Jobs that are executed by [Crafters](../worker/member/crafter/) and [Reviewers](../worker/member/reviewer/).

## Examples

- "Plan the next release of product A" — a planning Task. When completed, it produces child Tasks such as "add feature X", "add feature Y", and "fix bug Z."
- "Add dark mode support" — an implementation Task. It is broken down into Jobs: a Craft Job for a Crafter and Review Jobs for Reviewers.

## Collocations

- define (a Task for the system)
- break down (a Task into child Tasks or Jobs)
- complete (a Task when all its child Tasks or Jobs are done)

## Domain Rules

- A Task is complete when all of its child Tasks or Jobs are complete.
- A planning Task produces child Tasks as its deliverable.
- An implementation Task is broken down into Jobs.

## Related Concepts

- [Operator](../operator/) — defines the Task
- [Job](../job/) — a unit of work that a Member executes to fulfill a Task
