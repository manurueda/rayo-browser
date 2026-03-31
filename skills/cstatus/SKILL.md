---
name: cstatus
description: |
  Show status of all conductors (fix, feature, architect, guardian) in a unified dashboard. Use when the user types /cstatus.
allowed-tools:
  - Bash
  - Read
---

# /cstatus — Conductor Status Dashboard

Show the status of ALL conductor systems in a single unified view.

## What to do

Run all four status commands in parallel:

```bash
.fix/launch.sh status
.feature/launch.sh status
.architect/launch.sh status
.guardian/launch.sh status
```

Then present a unified dashboard as a markdown table:

```
## Conductor Dashboard

| System     | Profile    | Status    | Active Sessions | Details |
|------------|------------|-----------|-----------------|---------|
| Fix        | fix        | N running | session names   | phase info |
| Feature    | feature    | N running | session names   | phase info |
| Architect  | architect  | running/idle | session name | phase info |
| Guardian   | guardian   | running/idle | session name | current task |
```

Below the table, show details for any active sessions:
- For fix: slug, branch, phase, bugs being worked on
- For feature: slug, branch, phase, current module
- For architect: current phase (survey/audit/prescribe), artifacts generated
- For guardian: current task, work queue length

Keep it concise — one line per session, table format.
