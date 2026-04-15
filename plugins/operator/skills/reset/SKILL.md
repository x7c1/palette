---
name: reset
description: Destructively reset all Palette runtime data. Removes all workflow records, workspaces, transcripts, artifacts, and blueprints.
user-invocable: true
---

# /palette:reset

Destructively reset all Palette runtime data.

**This is a destructive operation.** All workflow data (workspaces, transcripts, artifacts, blueprints, and their database records) will be permanently deleted.

## Step 1: Check Orchestrator Is Stopped

```bash
curl -s -o /dev/null -w '%{http_code}' http://127.0.0.1:7100/health
```

If the response is `200`, tell the Operator:

> Orchestrator is still running. Please run `/palette:shutdown` first.

Then stop.

## Step 2: Dry-Run Preview

```bash
~/.config/palette/repo/target/release/palette admin reset --dry-run
```

If the binary does not exist, tell the Operator to run `/palette:setup` first and stop.

Present the dry-run output to the Operator showing all workflows and files that would be deleted.

## Step 3: Confirm and Execute

Warn the Operator explicitly:

> **This will permanently delete ALL runtime data** (workflows, workspaces, transcripts, artifacts, blueprints). This cannot be undone.

Ask for explicit confirmation before proceeding.

Once confirmed, run:

```bash
~/.config/palette/repo/target/release/palette admin reset --yes
```

## Step 4: Report Result

Show the Operator what was deleted (workflows removed, files cleaned up).
