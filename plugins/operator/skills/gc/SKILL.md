---
name: gc
description: Garbage-collect completed and failed workflows and their runtime artifacts. Previews targets with dry-run before executing.
user-invocable: true
---

# /palette:gc

Garbage-collect stale workflows and their runtime artifacts.

## Step 1: Check Orchestrator Is Stopped

```bash
curl -s -o /dev/null -w '%{http_code}' http://127.0.0.1:7100/health
```

If the response is `200`, tell the Operator:

> Orchestrator is still running. Please run `/palette:shutdown` first.

Then stop.

## Step 2: Dry-Run Preview

Build the command with any filters the Operator specified:

```bash
cd ~/.config/palette/repo && target/release/palette admin gc --dry-run [OPTIONS]
```

Available filter flags:

| Flag | Description |
|---|---|
| `--older-than-hours <N>` | Only target workflows older than N hours |
| `--workflow-id <ID>` | Target a specific workflow (repeatable) |
| `--include-active` | Include active/suspending workflows |

If the binary does not exist, tell the Operator to run `/palette:setup` first and stop.

Present the dry-run output to the Operator showing which workflows and files would be deleted.

If no matching workflows are found, report that and stop.

## Step 3: Confirm and Execute

Ask the Operator to confirm before proceeding.

Once confirmed, run:

```bash
cd ~/.config/palette/repo && target/release/palette admin gc --yes [OPTIONS]
```

Use the same filter flags from Step 2.

## Step 4: Report Result

Show the Operator what was deleted (workflows removed, files cleaned up).
