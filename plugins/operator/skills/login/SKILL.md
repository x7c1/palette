---
name: login
description: Refresh Claude authentication token for Palette workers. Guides operator through interactive login.
user-invocable: true
---

# /palette:login

Refresh or set up Claude authentication credentials for Palette worker containers.

On macOS, `.credentials.json` does not exist on the host filesystem (Claude Code uses the system Keychain). Worker containers require `.credentials.json`, so authentication must be performed inside a Linux container.

`claude auth login` requires interactive stdin (to paste the authorization code after browser authentication), so the Operator must run the Docker command themselves.

## Step 1: Prepare Auth Bundle Directory

```bash
mkdir -p ~/.config/palette/claude-auth-bundle/.claude
```

## Step 2: Instruct the Operator

Tell the Operator:

> Run the following command with the `!` prefix. It will display a URL — open it in your browser, authenticate, then paste the authorization code back into the terminal.
>
> ```
> ! docker run --rm -it -v ~/.config/palette/claude-auth-bundle/.claude:/home/agent/.claude palette-base:latest claude auth login
> ```
>
> After authentication completes, let me know.

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
