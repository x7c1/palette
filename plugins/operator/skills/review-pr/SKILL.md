---
name: review-pr
description: Start a standalone PR review. Selects perspectives, ensures the Orchestrator is running, and calls the review API.
user_invocable: true
---

# /palette:review-pr

Start a standalone PR review workflow. Guides the Operator through perspective selection and calls the Palette API.

## Step 1: Identify the Target PR

Ask the Operator which PR to review. Accept either:

- A PR URL (e.g., `https://github.com/owner/repo/pull/123`)
- An `owner/repo#number` reference (e.g., `haco-studio/stencil#42`)

Extract `owner`, `repo`, and `number` from the input.

## Step 2: Check Perspective Configuration

Read `~/.config/palette/repo/config/palette.toml` and look for `[[perspectives]]` entries.

If no `[[perspectives]]` entries exist, tell the Operator:

> Perspective の設定がありません。レビューを実行するには、設定ファイルに perspective を追加してください。
>
> 設定ファイル: `~/.config/palette/repo/config/palette.toml`
>
> 記述例:
> ```toml
> [perspectives_dirs]
> atelier = "/path/to/knowledge-base"
>
> [[perspectives]]
> name = "architecture"
> paths = ["atelier:architecture.md"]
>
> [[perspectives]]
> name = "type-safety"
> paths = ["atelier:type-safety.md"]
> ```
>
> 設定後、`/palette:doctor` で検証してください。

Then stop.

## Step 3: Select Perspectives

1. List all available perspectives from the config
2. Fetch the PR title and description:

```bash
gh pr view <number> --repo <owner>/<repo> --json title,body -q '.title + "\n" + .body'
```

3. Based on the PR content, recommend which perspectives to use. Present the recommendation:

> 以下の perspective でレビューします。変更があれば教えてください。
>
> 1. [x] architecture
> 2. [x] type-safety
> 3. [ ] performance

4. Wait for the Operator to confirm or adjust (e.g., "3 番も追加して", "全部でいい", "1 だけ外して")
5. Finalize the selected perspectives

## Step 4: Ensure Orchestrator is Running

Check if the Orchestrator is already running:

```bash
curl -s -o /dev/null -w '%{http_code}' http://127.0.0.1:7100/health
```

If not running, follow the `/palette:start` procedure:

1. Run `~/.config/palette/repo/target/release/palette doctor` — if it fails, stop and report
2. Read the tmux session name from `~/.config/palette/repo/config/palette.toml`
3. Launch via tmux:

```bash
tmux new-session -d -s <session_name> -n orchestrator \
  'cd ~/.config/palette/repo && target/release/palette start 2>&1 | tee data/palette.log'
```

4. Poll health for up to 30 seconds. If it times out, stop and report

## Step 5: Call the API

```bash
curl -s -X POST http://127.0.0.1:7100/workflows/start-pr-review \
  -H 'Content-Type: application/json' \
  -d '{
    "owner": "<owner>",
    "repo": "<repo>",
    "number": <number>,
    "reviewers": [
      {"perspective": "<perspective-1>"},
      {"perspective": "<perspective-2>"}
    ]
  }'
```

## Step 6: Report Result

On success, parse the JSON response and tell the Operator:

> PR レビューを開始しました。
> - Workflow ID: `<workflow_id>`
> - タスク数: `<task_count>`
>
> 進捗は `/palette:status` で確認できます。

On error, parse the error response and explain the cause (e.g., perspective name mismatch, empty reviewers).
