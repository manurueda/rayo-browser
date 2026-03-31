---
name: architect
description: |
  Launch the architect conductor for a strategic codebase audit. Use when the user types /architect.
  Examples: "/architect", "/architect status", "/architect stop"
allowed-tools:
  - Bash
  - Read
---

# /architect — Strategic Codebase Audit

You are launching the architect conductor via `.architect/launch.sh`. This runs a read-only audit pipeline (collect → survey → audit → prescribe) that produces CENSUS.md, AUDIT.md, and PLAN.md.

## Parse the user's input

The argument after `/architect` determines the action:

- **`status`** — check progress
- **`stop`** — stop the architect
- **`reset`** — stop + cleanup
- **No argument** or **anything else** — launch the architect

## Launching

1. Run: `.architect/launch.sh`
2. Confirm launch to the user

## Checking status

1. Run: `.architect/launch.sh status`
2. Summarize: which phase (survey/audit/prescribe), any artifacts generated

## Stopping

1. Run: `.architect/launch.sh stop`

## Important

- Only one architect runs at a time
- The architect NEVER modifies code — it only produces reports
- Approved proposals are handed off to the Guardian via WORK_QUEUE.md
- Artifacts: `.architect/CENSUS.md`, `.architect/AUDIT.md`, `.architect/PLAN.md`
