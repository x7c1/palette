# Claude Login Bootstrap (macOS/Linux)

Run `claude login` once in the `palette` bootstrap container to create runtime auth artifacts.  
This flow works on both macOS and Linux.

## Prerequisites

- `~/.claude/CLAUDE.md` exists
- `~/.claude/settings.json` exists
- (Only when needed) extra CA certificates are placed in `~/.config/palette/certs/`

## Minimal Steps

```bash
cd <palette-repo-root>
HOST_HOME=$HOME docker compose up -d claude-code
docker exec -it palette-claude-code-1 bash
claude login
```

After login completes, verify auth bundle propagation:

```bash
cd <palette-repo-root>
./tests/e2e/check-linux-bootstrap-auth-bundle.sh
```

Done when this appears:

`PASS: bootstrap bundle export and mount propagation succeeded`

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
