---
description: Stop Palette and clean up all resources
---

# palette-stop

Stop Palette and clean up all associated resources (containers, tmux session, DB files).

## Instructions

- Read the PID from `data/palette.pid`
  - If the file does not exist, warn that Palette does not appear to be running but continue with cleanup
- Kill the Palette process using the PID
  - Use `kill` (SIGTERM); if the process does not exit within 5 seconds, use `kill -9`
- Run `scripts/reset.sh` to clean up containers, tmux session, and DB files
- Remove `data/palette.pid`
- Report that Palette has been stopped and cleanup is complete
