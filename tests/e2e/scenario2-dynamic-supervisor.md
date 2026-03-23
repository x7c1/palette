# Scenario 2: Dynamic Supervisor Spawn

Verify that supervisors are dynamically spawned per composite task and destroyed on completion.

## Task Tree

```
root (dynamic-supervisor)        → Leader spawned
├── phase-a (pure composite)     → Leader spawned
│   └── craft (craft job + review child)
│       └── review (review job)
└── phase-b (pure composite, depends_on: phase-a)  → Leader spawned after phase-a completes
    └── craft (craft job + review child)
        └── review (review job)
```

## Fixture

`tests/e2e/fixtures/dynamic-supervisor.yaml`

## Steps

- Start Palette using `palette-start`
- Approve the fixture Blueprint using `palette-approve tests/e2e/fixtures/dynamic-supervisor.yaml`
- Confirm the response shows `task_count: 7` (root + phase-a + phase-a/craft + phase-a/craft/review + phase-b + phase-b/craft + phase-b/craft/review)
- Check `data/state.json`: supervisors should contain 2 entries (root + phase-a), each with a `task_id`
- Wait for phase-a/craft to complete
- Check `data/state.json`: phase-a supervisor should be gone, phase-b supervisor should appear (total: 2)
- Wait for phase-b/craft to complete
- Check `data/state.json`: all supervisors should be gone (total: 0)
- Confirm "workflow completed" in logs
- Stop Palette using `palette-stop`

## Automated Script

```bash
tests/e2e/run-scenario2.sh
```

## Expected Results

- After workflow start: 2 supervisors (root + phase-a)
- After phase-a completes: 2 supervisors (root + phase-b), phase-a supervisor destroyed
- After workflow completes: 0 supervisors, all destroyed
