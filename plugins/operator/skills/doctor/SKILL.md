---
name: doctor
description: Check Palette prerequisites and system health. Reports which tools and images are available.
user-invocable: true
---

# /palette:doctor

Check whether all prerequisites for running Palette are met.

## Instructions

Run the doctor command:

```bash
~/.config/palette/repo/target/release/palette doctor
```

Parse the JSON output and present the results to the Operator in a readable format:

- Show each check with its status (pass/fail) and message
- If all checks pass, confirm the system is ready
- If any checks fail, explain what is missing and suggest how to fix it (e.g., install the tool, start Docker, run `/palette:setup` to rebuild)

If the binary does not exist at `~/.config/palette/repo/target/release/palette`, tell the Operator to run `/palette:setup` first.
