# Orchestrator

## Definition

The Orchestrator is the infrastructure layer of Palette. It manages the lifecycle of [Workers](../worker/), routes messages between them, and enforces [Job](../job/) state transitions according to predefined rules. The Orchestrator makes no judgments — it applies rules mechanically.

All communication between Workers passes through the Orchestrator. No Worker communicates directly with another.

## Examples

- The Orchestrator spawns a [Crafter](../worker/member/crafter/) when a Craft Job is ready to begin.
- The Orchestrator transitions a Craft Job from "in progress" to "in review" when the Crafter finishes, and spawns [Reviewers](../worker/member/reviewer/) for the associated Review Jobs.
- The Orchestrator delivers a permission prompt notification from a Crafter to the [Leader](../worker/supervisor/leader/).
- The Orchestrator despawns a Worker when its Job is complete.

## Collocations

- spawn (a Worker)
- despawn (a Worker when its Job is complete)
- route (a message between Workers)
- transition (a Job's state according to rules)

## Domain Rules

- The Orchestrator does not make decisions. All state transitions follow predefined rules.
- All inter-Worker communication is mediated by the Orchestrator.

## Related Concepts

- [Worker](../worker/) — the Orchestrator manages Worker lifecycle
- [Job](../job/) — the Orchestrator tracks Job state and enforces transitions
