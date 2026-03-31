---
name: feature
description: |
  Launch a feature conductor to build a new feature end-to-end. Use when the user types /feature followed by a feature description.
  Examples: "/feature add dark mode toggle", "/feature status", "/feature stop my-slug"
allowed-tools:
  - Bash
  - Read
---

# /feature — Feature Development Pipeline

You are launching a feature conductor via `.feature/launch.sh`. This runs an autonomous pipeline (spec → implement → test → break → UI test → merge) in an isolated agent-deck session.

## Parse the user's input

The argument after `/feature` determines the action:

- **`status`** or **`status <slug>`** — check progress
- **`stop`** or **`stop <slug>`** — stop conductors
- **`reset`** — stop all + cleanup worktrees/branches
- **Anything else** — treat as a feature description, launch a new conductor

## Launching a feature

1. Run: `.feature/launch.sh start '<feature description>'`
   - The script auto-generates a slug from the description
   - Each conductor runs independently on its own `feature/<slug>` branch
   - Multiple conductors can run in parallel on different features

2. Confirm launch to the user with the monitoring command

## Checking status

1. Run: `.feature/launch.sh status` (all) or `.feature/launch.sh status <slug>` (one)
2. Summarize the output concisely

## Stopping

1. Run: `.feature/launch.sh stop` (all) or `.feature/launch.sh stop <slug>` (one)
2. For full cleanup: `.feature/launch.sh reset`

## Important

- Each `/feature` call with a description launches a NEW parallel conductor
- Conductors are fully autonomous — they do not need babysitting
- The conductor merges to main and pushes when all checks pass
- To monitor a running conductor: `.feature/launch.sh status <slug>`
