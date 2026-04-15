---
name: shutdown
description: Shut down the running Palette Orchestrator. Sends a graceful shutdown request and waits for the process to stop.
user-invocable: true
---

# /palette:shutdown

Gracefully shut down the running Palette Orchestrator.

## Step 1: Health Check

```bash
curl -s -o /dev/null -w '%{http_code}' http://127.0.0.1:7100/health
```

If the response is NOT `200`, tell the Operator "Orchestrator is not running." and stop.

## Step 2: Shut Down

```bash
~/.config/palette/repo/target/release/palette shutdown
```

If the binary does not exist, tell the Operator to run `/palette:setup` first and stop.

The command sends a POST to `/shutdown` and polls `/health` until the server stops responding.

## Step 3: Report Result

If the command succeeds, tell the Operator:

> Orchestrator has been shut down.

If the command fails, show the error output to the Operator.
