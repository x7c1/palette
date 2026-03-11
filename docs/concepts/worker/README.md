# Worker

## Definition

A Worker is any execution unit managed by the [Orchestrator](../orchestrator/). The Orchestrator spawns, monitors, and despawns Workers. All [Supervisors](supervisor/) and [Members](member/) are Workers.

Worker is not a role — it describes the relationship to the Orchestrator. From the Orchestrator's perspective, all Workers are managed uniformly: they are spawned, they communicate through the Orchestrator, and they are despawned when their work is done.

## Examples

- A [Leader](supervisor/leader/) is a Worker that the Orchestrator spawns at the start of a [Task](../task/).
- A [Crafter](member/crafter/) is a Worker that the Orchestrator spawns when a Craft [Job](../job/) is ready.
- A [Reviewer](member/reviewer/) is a Worker that the Orchestrator spawns when a Review Job is ready.
- The Orchestrator checks whether all Workers are idle.

## Collocations

- spawn (a Worker)
- despawn (a Worker)
- monitor (a Worker's status)

## Related Concepts

- [Orchestrator](../orchestrator/) — manages Worker lifecycle
- [Supervisor](supervisor/) — a Worker that oversees other Workers
- [Member](member/) — a Worker that executes a Job
