# Task

## Definition

A Task is a goal that the [Operator](../operator/) wants to achieve. It describes *what* should be accomplished, not *how* to accomplish it.

A Task can be broken down into child Tasks, forming a tree. A Task can also have a [Job](../job/) assigned to it — a Job defines who works on the Task and how. A Task that has been broken down into child Tasks is called a Composite Task.

Dependencies between Tasks are defined among siblings — Tasks that share the same parent. A dependent Task cannot begin until the Tasks it depends on are complete.

## Composite Task

A Composite Task is a Task that has child Tasks. A [Leader](../worker/supervisor/leader/) can be assigned to a Composite Task to supervise its child Tasks, handle decisions that arise during execution, and raise [Escalations](../escalation/) when a decision exceeds its confidence.

A Task can become a Composite Task through [Blueprint](../blueprint/) editing during a [Workflow](../workflow/) suspend — for example, when a Pending Task needs to be broken down into child Tasks before work begins.

## Completion

A Task is complete when all of its child Tasks are complete and its Job (if any) is done.

## Examples

- "Add feature X" — a Composite Task broken down into child Tasks: "plan feature X" and "implement feature X," where the latter depends on the former.
- "Implement the API endpoint" — a Task with a Craft Job assigned to a [Crafter](../worker/member/crafter/) and a Review Job assigned to a [Reviewer](../worker/member/reviewer/) (as separate child Tasks).
- "Plan feature X" depends on nothing; "implement feature X" depends on "plan feature X." Both are children of "add feature X."

## Collocations

- define (a Task)
- break down (a Task into child Tasks)
- complete (a Task)
- depend on (a sibling Task)

## Domain Rules

- A Task is complete when all of its child Tasks are complete and its Job (if any) is done.
- A Task has at most one Job.
- Dependencies are defined among sibling Tasks only.
- A dependent Task cannot begin until the Tasks it depends on are complete.
- A Task can have both child Tasks and a Job at the same time.

## Related Concepts

- [Operator](../operator/) — defines the Task
- [Job](../job/) — a work assignment on a Task
- [Plan](../plan/) — describes the scope and approach for the Task
- [Leader](../worker/supervisor/leader/) — supervises a Composite Task
- [Blueprint](../blueprint/) — defines a Task tree
