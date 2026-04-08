---
name: start
description: Start the Palette Orchestrator. Checks health first and skips if already running. Launches via tmux.
user_invocable: true
---

# /palette:start

Start the Palette Orchestrator. If it is already running, report that and do nothing.

## Step 1: Health Check

```bash
curl -s -o /dev/null -w '%{http_code}' http://127.0.0.1:7100/health
```

If the response is `200`, tell the Operator "Orchestrator is already running." and stop.

## Step 2: Check Prerequisites

```bash
~/.config/palette/repo/target/release/palette doctor
```

If the binary does not exist, tell the Operator to run `/palette:setup` first and stop.

If any check fails, show the failures and stop. Do not attempt to start.

## Step 3: Start via tmux

Read the tmux session name from `~/.config/palette/repo/config/palette.toml` (the `[tmux] session_name` field). Default to `"palette"` if not found.

```bash
tmux new-session -d -s <session_name> -n orchestrator \
  'cd ~/.config/palette/repo && target/release/palette start 2>&1 | tee data/palette.log'
```

## Step 4: Wait for Health

Poll the health endpoint until it responds, up to 30 seconds:

```bash
for i in $(seq 1 30); do
  if curl -s -o /dev/null -w '%{http_code}' http://127.0.0.1:7100/health 2>/dev/null | grep -q 200; then
    echo "Orchestrator started successfully."
    exit 0
  fi
  sleep 1
done
echo "Timed out waiting for Orchestrator to start."
exit 1
```

If the health check succeeds, report success to the Operator with the tmux session name.

If it times out, tell the Operator to check the log at `~/.config/palette/repo/data/palette.log` for errors.
