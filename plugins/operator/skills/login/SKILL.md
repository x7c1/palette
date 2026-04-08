---
name: login
description: Refresh Claude authentication token for Palette workers. Runs bootstrap login and syncs credentials.
user-invocable: true
---

# /palette:login

Refresh or set up Claude authentication credentials for Palette worker containers.

On macOS, `.credentials.json` does not exist on the host filesystem (Claude Code uses the system Keychain). Worker containers require `.credentials.json`, so authentication must be performed inside a Linux bootstrap container. This skill automates the entire flow — the Operator only needs to open a URL in their browser.

## Step 1: Start Bootstrap Container

```bash
docker compose -f ~/.config/palette/repo/docker-compose.yml up -d claude-code
```

If this fails, tell the Operator:

> Failed to start bootstrap container. Ensure Docker is running and `~/.config/palette/repo/docker-compose.yml` exists.

Then stop.

## Step 2: Run claude auth login

Run the login command in the background and capture its output:

```bash
docker exec palette-claude-code-1 claude auth login 2>&1
```

This command blocks until authentication completes. Run it with a long timeout (up to 5 minutes) or in the background.

The command will output a line like:

```
If the browser didn't open, visit: https://claude.com/cai/oauth/authorize?...
```

Extract the URL from that line.

## Step 3: Present URL to Operator

Tell the Operator:

> Open this URL in your browser to authenticate:
>
> `<extracted URL>`
>
> After completing authentication in the browser, wait a moment for the process to finish.

Wait for the `claude auth login` command to complete.

If it succeeds (exit code 0), proceed to Step 4.

If it fails or times out, tell the Operator:

> Authentication did not complete. Please try again with `/palette:login`.

Then stop.

## Step 4: Sync Auth Bundle

```bash
~/.config/palette/repo/scripts/sync-bootstrap-auth-bundle.sh
```

If it fails, tell the Operator:

> Auth bundle sync failed. Check the error output above and try running the script manually:
> `~/.config/palette/repo/scripts/sync-bootstrap-auth-bundle.sh`

Then stop.

## Step 5: Restart Auth-Error Workers

Check if the Orchestrator is running:

```bash
curl -s -o /dev/null -w '%{http_code}' http://127.0.0.1:7100/health
```

If the Orchestrator is running (`200`), check for workers that may need restarting. Fetch the Orchestrator log for recent `authentication_error` entries:

```bash
tail -100 ~/.config/palette/repo/data/palette.log | grep -i 'authentication_error\|auth.*error'
```

If auth errors were found, tell the Operator:

> Credentials updated. Workers with authentication errors will be detected and restarted by the Orchestrator's crash recovery on their next monitoring cycle.

If no auth errors were found (e.g., this was a proactive token refresh), tell the Operator:

> Credentials updated. No authentication errors detected in recent logs.

## Step 6: Report Result

Tell the Operator:

> Authentication complete.
> - Auth bundle synced to `~/.config/palette/claude-auth-bundle/`
> - Worker containers will pick up new credentials automatically (bind mount).
