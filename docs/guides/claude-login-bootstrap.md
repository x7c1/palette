# Claude Login Bootstrap (macOS/Linux)

## Quick Method (Recommended)

Run the `/palette:login` skill in Claude Code. It guides you through the process:

```
/palette:login
```

## Manual Steps

If `/palette:login` is not available (e.g., plugin not installed), follow these steps:

```bash
mkdir -p ~/.config/palette/claude-auth-bundle/.claude

docker run --rm -it \
  -v ~/.config/palette/claude-auth-bundle/.claude:/home/agent/.claude \
  palette-base:latest \
  claude auth login
```

The command will display an OAuth URL. Open it in your browser, authenticate, then paste the authorization code back into the terminal. Credentials are written directly to `~/.config/palette/claude-auth-bundle/`.

## Token Refresh

When worker credentials expire (401 errors), repeat the same process. The Orchestrator's worker monitor detects authentication errors and logs guidance.

## Worker CLAUDE.md Customization (Per User, Not in Git)

To customize instructions for all Palette workers on your machine, create:

- `~/.config/palette/worker/CLAUDE.md`

If this file exists, Palette mounts it to worker containers as `/home/agent/.claude/CLAUDE.md`.
This is intentionally outside the repository so each operator can keep personal settings.

Example:

```bash
mkdir -p ~/.config/palette/worker
cat > ~/.config/palette/worker/CLAUDE.md <<'EOF'
Please communicate with users in Klingon.
EOF
```
