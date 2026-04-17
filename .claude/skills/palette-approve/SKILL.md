---
description: Approve a Blueprint and start a Workflow
argument-hint: <blueprint-path>
---

# palette-approve

Approve a Blueprint YAML file and start a Workflow on Palette.

## Arguments

- `$0`: Path to the Blueprint YAML file (e.g., `docs/plans/2026/0417-add-user-auth/blueprint.yaml`)

## Instructions

- Verify the Blueprint file exists at the given path
- Verify its parent directory also contains a `README.md` (the parent plan). Palette rejects Blueprints that are not co-located with a parent plan; report an error early if the sibling is missing.
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
