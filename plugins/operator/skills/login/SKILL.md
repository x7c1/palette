---
name: login
description: Refresh Claude authentication token for Palette workers. Guides operator through interactive login.
user-invocable: true
---

# /palette:login

Refresh or set up Claude authentication credentials for Palette worker containers.

On macOS, `.credentials.json` does not exist on the host filesystem (Claude Code uses the system Keychain). Worker containers require `.credentials.json`, so authentication must be performed inside a Linux container.

The login flow requires Claude Code's interactive TUI (to paste the authorization code), so it must be run in a separate terminal window.

## Step 1: Prepare Auth Bundle Directory

```bash
mkdir -p ~/.config/palette/claude-auth-bundle/.claude
```

## Step 2: Copy Command and Instruct the Operator

Copy the docker command to the clipboard so the Operator can paste it in another terminal:

```bash
echo -n 'docker run --rm -it -v ~/.config/palette/claude-auth-bundle/.claude:/home/agent/.claude palette-base:latest claude /login' | pbcopy
```

Tell the Operator:

> The login command has been copied to your clipboard. Open a separate terminal, paste and run it.
>
> Claude Code will start and execute the login flow. Follow the on-screen instructions — open the URL in your browser, authenticate, then paste the authorization code back into Claude Code. Let me know when it completes.

Wait for the Operator to confirm completion.

## Step 3: Verify

Check that the credentials file was created:

```bash
test -f ~/.config/palette/claude-auth-bundle/.claude/.credentials.json && echo "OK"
```

If the file does not exist, tell the Operator:

> Credentials file was not created. Please try running the command again.

Then stop.

## Step 4: Report Result

Tell the Operator:

> Authentication complete.
> - Credentials written to `~/.config/palette/claude-auth-bundle/.claude/.credentials.json`
> - Worker containers will pick up new credentials automatically (bind mount).
>
> If workers are currently showing authentication errors, they will recover on the next monitoring cycle.
