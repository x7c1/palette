# Palette Concepts

Palette is an orchestration system that enables autonomous [Workers](worker/) to collaborate without constant human oversight, by having [Supervisors](worker/supervisor/) oversee [Members](worker/member/).

## The Problem

Autonomous Workers are capable of producing high-quality work when given proper instructions. However, they cannot work independently — they require a human to stay present throughout the session, answering permission prompts, confirming approaches, and reviewing results. This constant human involvement prevents Workers from realizing their full potential.

## The Approach

An [Operator](operator/) tells Palette what to achieve — a [Task](task/). Palette plans the work by having a [Crafter](worker/member/crafter/) produce a [Blueprint](blueprint/) that breaks the goal into child Tasks and [Jobs](job/). The Operator then starts a [Workflow](workflow/) from the Blueprint, and Palette orchestrates a team of Workers to complete it.

Palette replaces the human-in-the-loop with a Supervisor-in-the-loop in two ways:

1. **Supervised execution**: Instead of the Operator staying present to approve actions and make judgment calls, a Supervisor takes on that role. The [Permission Supervisor](worker/supervisor/permission-supervisor/) handles permission prompts from [Members](worker/member/) during execution, while the [Review Integrator](worker/review-integrator/) consolidates findings from multiple [Reviewers](worker/member/reviewer/) into a single verdict.
2. **Automated review cycle**: Instead of the Operator reviewing work and requesting revisions, Reviewers review the [Crafter](worker/member/crafter/)'s work. The cycle of implementation and review repeats until quality criteria are met — all without human involvement.

The Operator only intervenes when a Supervisor encounters a decision beyond its confidence — an [Escalation](escalation/).

The system is built on a separation between two concerns:

- **Infrastructure automation**: Mechanical rule application — routing messages, tracking Task and Job state, enforcing lifecycle transitions. No judgments are made. This is the role of the [Orchestrator](orchestrator/).
- **Runtime decision-making**: The decisions the Operator would otherwise make — approving actions, evaluating review results, and deciding when to escalate. This is the role of Supervisors.
