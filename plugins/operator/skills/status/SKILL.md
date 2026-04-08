---
name: status
description: Check Palette workflow and job progress. Shows running workflows and their job statuses.
user_invocable: true
---

# /palette:status

Show the current status of Palette workflows and jobs.

## Step 1: Check Orchestrator

```bash
curl -s -o /dev/null -w '%{http_code}' http://127.0.0.1:7100/health
```

If the Orchestrator is not running (no `200` response), tell the Operator:

> Orchestrator が起動していません。`/palette:start` で起動してください。

Then stop.

## Step 2: List Workflows

```bash
curl -s http://127.0.0.1:7100/workflows | jq .
```

Present the workflows to the Operator in a table:

| Workflow ID | Status | Started At |
|---|---|---|

If no workflows exist, tell the Operator that no workflows have been started yet.

## Step 3: Show Job Details

If there are active workflows, fetch the jobs:

```bash
curl -s 'http://127.0.0.1:7100/jobs' | jq .
```

Present the jobs grouped by type (review, review_integrate, craft, etc.):

| Job ID | Type | Title | Status | Assignee |
|---|---|---|---|---|

## Step 4: Summary

Provide a brief summary:

- Total workflows (active / completed / suspended)
- Total jobs by status (in_progress / done / todo)
- If all jobs in a workflow are done, note that the workflow is complete
