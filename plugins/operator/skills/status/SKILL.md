---
name: status
description: Check Palette workflow and job progress. Shows running workflows and their job statuses.
user-invocable: true
---

# /palette:status

Show the current status of Palette workflows and jobs.

## Step 1: Check Orchestrator

```bash
curl -s -o /dev/null -w '%{http_code}' http://127.0.0.1:7100/health
```

If the Orchestrator is not running (no `200` response), tell the Operator:

> Orchestrator is not running. Start it with `/palette:start`.

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

## Step 4: Check for Authentication Errors

Check the Orchestrator log for recent authentication errors:

```bash
tail -200 ~/.config/palette/repo/data/palette.log | grep -i 'authentication error detected'
```

If any matches are found, display a prominent warning:

> **Authentication Error Detected**
>
> One or more workers have expired credentials. Run `/palette:login` to refresh the auth token.

## Step 5: Summary

Provide a brief summary:

- Total workflows (active / completed / suspended)
- Total jobs by status (in_progress / done / todo)
- If all jobs in a workflow are done, note that the workflow is complete
- If authentication errors were detected in Step 4, remind the Operator to run `/palette:login`
