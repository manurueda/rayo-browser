---
name: fix
description: |
  Launch a parallel TDD fix pipeline for a bug. Use when the user types /fix followed by a bug description.
  Examples: "/fix auto mode 500 error", "/fix legends static in office", "/fix status", "/fix stop my-slug"
allowed-tools:
  - Bash
  - Read
---

# /fix — TDD Fix Pipeline

You are launching a fix conductor via `.fix/launch.sh`. This runs an autonomous TDD pipeline (diagnose -> red -> green -> adversarial -> verify) in an isolated agent-deck session.

## Parse the user's input

The argument after `/fix` determines the action:

- **`status`** or **`status <slug>`** — check progress
- **`stop`** or **`stop <slug>`** — stop conductors
- **`reset`** — stop all + cleanup worktrees/branches
- **Anything else** — treat as a bug description, launch a new conductor

## Launching a fix

1. Run: `.fix/launch.sh start '<bug description>'`
   - The script auto-generates a slug from the description
   - Each conductor runs independently on its own `fix/<slug>` branch
   - Multiple conductors can run in parallel on different bugs

2. Confirm launch to the user with the monitoring command

## Checking status

1. Run: `.fix/launch.sh status` (all) or `.fix/launch.sh status <slug>` (one)
2. Summarize the output concisely

## Stopping

1. Run: `.fix/launch.sh stop` (all) or `.fix/launch.sh stop <slug>` (one)
2. For full cleanup: `.fix/launch.sh reset`

## Important

- Each `/fix` call with a bug description launches a NEW parallel conductor
- Conductors are fully autonomous — they do not need babysitting
- The conductor does NOT merge to main or push — it leaves the branch for the user
- To monitor a running conductor: `.fix/launch.sh status <slug>`
