---
description: Approve a Blueprint and start a Workflow
argument-hint: <blueprint-path>
---

# palette-approve

Approve a Blueprint YAML file and start a Workflow on Palette.

## Arguments

- `$0`: Path to the Blueprint YAML file (e.g., `data/blueprints/task-tree-cascade.yaml`)

## Instructions

- Verify the Blueprint file exists at the given path
- Resolve the path to an absolute path
- Send a POST request to start the Workflow:
  ```
  curl -sf -X POST http://127.0.0.1:7100/workflows/start \
    -H "Content-Type: application/json" \
    -d '{"blueprint_path": "<absolute-path>"}'
  ```
- Parse the JSON response to extract `workflow_id` and `task_count`
- Report the Workflow ID and the number of tasks created
- If the request fails, show the error response body for debugging
- Suggest using `palette-status` to monitor progress
