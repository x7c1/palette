---
description: Show status of Palette Workflows, Jobs, and Agents
---

# palette-status

Display the current status of running Workflows, Jobs, and Agents on Palette.

## Instructions

- Fetch Job status from Palette:
  ```
  curl -sf http://127.0.0.1:7100/jobs
  ```
  - Display each Job's id, type, title, status, and assignee in a table
  - If the request fails, report that Palette may not be running

- Read Agent state from `data/state.json` (if it exists):
  - Show supervisors: id, role, status, container_id (first 12 chars)
  - Show members: id, supervisor_id, status, container_id (first 12 chars)
  - If the file does not exist, note that no Agent state is available

- Check Docker containers:
  ```
  docker ps --filter label=palette.managed=true --format "table {{.ID}}\t{{.Names}}\t{{.Status}}"
  ```
  - If no containers are running, note this

- Determine overall Workflow status:
  - If all craft Jobs have status `done` → report "Workflow complete"
  - If any Jobs are `working` or `assigned` → report "Workflow in progress"
  - If any Jobs are `ready` → report "Workflow has ready Jobs waiting for assignment"
  - Otherwise → report the current state summary
