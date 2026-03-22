# Scenario 1: Task Tree Cascade

Verify that dependent tasks are resolved correctly through the task tree, including the craft → review cycle within each step.

## Task Tree

```
root (e2e/task-tree-cascade)
├── step-a (composite)
│   └── craft (craft job + review child)
│       └── review (review job)
└── step-b (composite, depends_on: step-a)
    └── craft (craft job + review child)
        └── review (review job)
```

## Fixture

`tests/e2e/fixtures/task-tree-cascade.yaml`

## Steps

- Start Palette using `palette-start`
- Approve the fixture Blueprint using `palette-approve tests/e2e/fixtures/task-tree-cascade.yaml`
- Confirm the response shows `workflow_id` and `task_count: 7` (root + step-a + step-a/craft + step-a/craft/review + step-b + step-b/craft + step-b/craft/review)
- Run `palette-status` to check initial state:
  - step-a/craft should have a Job in `todo` status
  - step-b should have no Jobs yet (its task is `pending` because it depends on step-a)
- Wait for step-a/craft's Job to complete (craft → in_review → review activated → review done → craft done)
- After step-a completes, verify step-b/craft's Job appears with `todo` status
- Wait for step-b/craft's Job to complete
- Run `palette-status` and confirm "Workflow complete"
- Stop Palette using `palette-stop`

## Automated Script

```bash
tests/e2e/run-scenario1.sh
```

The script automates the full flow with stall detection and diagnostic output on failure.

## Expected Results

- step-a/craft Job is created immediately in `todo` status
- step-b Jobs are NOT created until step-a completes (both craft and review)
- After step-a completes, step-b/craft Job is created in `todo` status
- After both steps complete, all craft Jobs have status `done`
