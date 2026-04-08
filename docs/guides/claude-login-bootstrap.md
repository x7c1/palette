# Claude Login Bootstrap (macOS/Linux)

## Quick Method (Recommended)

Run the `/palette:login` skill in Claude Code. It automates the entire flow — you only need to open a URL in your browser:

```
/palette:login
```

## Manual Steps

If `/palette:login` is not available (e.g., plugin not installed), follow these steps:

### 1. Start the Bootstrap Container

```bash
cd ~/.config/palette/repo
HOST_HOME=$HOME docker compose up -d claude-code
```

### 2. Run Login

```bash
docker exec palette-claude-code-1 claude auth login
```

The command will display an OAuth URL. Open it in your browser and complete authentication.

### 3. Sync Auth Bundle

After login completes:

```bash
cd ~/.config/palette/repo
scripts/sync-bootstrap-auth-bundle.sh
```

Done when this appears:

`PASS: synced auth bundle from <container> -> <output_dir>`

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
