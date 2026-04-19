---
name: approve
description: Approve a Blueprint and start a Workflow on Palette
user-invocable: true
argument-hint: "[blueprint-path]"
---

# /palette:approve

Approve a Blueprint YAML file and start a Workflow on Palette.

## Arguments

- `$0` (optional): Path to the Blueprint YAML file (e.g., `docs/plans/2026/0417-add-user-auth/blueprint.yaml`)
  - When omitted, use the Blueprint path that `/palette:plan` just generated in this conversation. If no such path is present in context, ask the Operator for one before proceeding.

## Instructions

- Determine the Blueprint path:
  - If `$0` is given, use it
  - Otherwise, reuse the most recently generated `blueprint.yaml` path from the current conversation (typically produced by a preceding `/palette:plan` run)
- Verify the Blueprint file exists
- Resolve the path to an absolute path
- Run the validate endpoint first so an invalid Blueprint is caught before a Workflow is even attempted:
  ```
  curl -sf -X POST http://127.0.0.1:7100/blueprints/validate \
    -H "Content-Type: application/json" \
    -d '{"blueprint_path": "<absolute-path>"}'
  ```
  - 200 with `valid: false` — surface the `errors[]` to the Operator and stop. Do not call `/workflows/start` until the Blueprint is fixed (typically by returning to `/palette:plan`)
  - 404 — report the path problem to the Operator and stop
  - 200 with `valid: true` — proceed to the next step
- Send a POST request to start the Workflow:
  ```
  curl -sf -X POST http://127.0.0.1:7100/workflows/start \
    -H "Content-Type: application/json" \
    -d '{"blueprint_path": "<absolute-path>"}'
  ```
- Parse the JSON response to extract `workflow_id` and `task_count`
- Report the Workflow ID and the number of tasks created
- If the request fails, show the error response body for debugging
- Suggest using `/palette:status` to monitor progress
