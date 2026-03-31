---
name: guardian
description: |
  Launch or manage the guardian — a persistent autonomous cleanup agent. Use when the user types /guardian.
  Examples: "/guardian", "/guardian status", "/guardian stop"
allowed-tools:
  - Bash
  - Read
---

# /guardian — Persistent Cleanup Agent

You are managing the guardian conductor via `.guardian/launch.sh`. The guardian runs continuously, scanning for SRP/DRY/DI violations, test gaps, and dead code, then fixing them autonomously.

## Parse the user's input

The argument after `/guardian` determines the action:

- **`status`** — check progress and current task
- **`stop`** — stop the guardian
- **`reset`** — stop + cleanup worktrees/branches/state
- **No argument** or **`start`** — launch the guardian

## Launching

1. Run: `.guardian/launch.sh`
2. Confirm launch to the user

## Checking status

1. Run: `.guardian/launch.sh status`
2. Summarize: current task, work queue length, recent completions

## Stopping

1. Run: `.guardian/launch.sh stop`

## Important

- Only one guardian runs at a time — it's a persistent loop
- The guardian auto-merges fixes to main and pushes
- Work queue: `.guardian/WORK_QUEUE.md`
- The guardian does NOT build features — it only cleans and hardens
