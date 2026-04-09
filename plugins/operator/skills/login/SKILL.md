---
name: login
description: Refresh Claude authentication token for Palette workers. Runs bootstrap login and syncs credentials.
user-invocable: true
---

# /palette:login

Refresh or set up Claude authentication credentials for Palette worker containers.

On macOS, `.credentials.json` does not exist on the host filesystem (Claude Code uses the system Keychain). Worker containers require `.credentials.json`, so authentication must be performed inside a Linux container. This skill automates the entire flow — the Operator only needs to open a URL in their browser.

## Step 1: Run claude auth login

Ensure the auth bundle directory exists, then run the login command in a temporary container:

```bash
mkdir -p ~/.config/palette/claude-auth-bundle/.claude
```

```bash
docker run --rm \
  -v ~/.config/palette/claude-auth-bundle/.claude:/home/agent/.claude \
  palette-base:latest \
  claude auth login
```

This command blocks until authentication completes. Run it with a long timeout (up to 5 minutes) or in the background.

The command will output a line like:

```
If the browser didn't open, visit: https://claude.com/cai/oauth/authorize?...
```

Extract the URL from that line.

## Step 2: Present URL to Operator

Tell the Operator:

> Open this URL in your browser to authenticate:
>
> `<extracted URL>`
>
> After completing authentication in the browser, wait a moment for the process to finish.

Wait for the `claude auth login` command to complete.

If it succeeds (exit code 0), proceed to Step 3.

If it fails or times out, tell the Operator:

> Authentication did not complete. Please try again with `/palette:login`.

Then stop.

## Step 3: Verify

Check that the credentials file was created:

```bash
test -f ~/.config/palette/claude-auth-bundle/.claude/.credentials.json && echo "OK"
```

If the file does not exist, tell the Operator:

> Credentials file was not created. Please try again with `/palette:login`.

Then stop.

## Step 4: Report Result

Tell the Operator:

> Authentication complete.
> - Credentials written to `~/.config/palette/claude-auth-bundle/.claude/.credentials.json`
> - Worker containers will pick up new credentials automatically (bind mount).
>
> If workers are currently showing authentication errors, they will recover on the next monitoring cycle.
