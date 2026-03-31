---
name: modular-delivery
description: Use when planning, reviewing, or implementing architectural changes that should stay modular, spec-driven, terminal-first, and explicit about SRP, DI, DRY, ports, cache invalidation, and rollout boundaries.
---

Use this skill for spec-first architecture work that should land as small, composable modules instead of broad, mixed-scope edits.

## Core Rules

- Keep one change focused on one capability slice or one tightly related rollout.
- Give each module one reason to change.
- Separate policy from mechanism.
- Depend on explicit ports or collaborators, not hidden globals.
- Cache only durable inputs; rebuild mutable overlays.
- Prefer terminal validation and same-process checks over UI-only confirmation.
- Treat OpenSpec as a terminal workflow, not as passive documentation.

## Workflow

**MANDATORY: Execute every numbered step below in order. Do not skip any step. If OpenSpec is detected in step 2, steps 3, 4, and 6 are required — not optional. Complete each step before moving to the next.**

1. Inspect the active repo instructions first.
   - Read `CLAUDE.md`, `AGENTS.md`, or equivalent project guidance before proposing structure changes.
   - Identify the real boundaries already present in the codebase.
2. Detect OpenSpec from the terminal.
   - Verify the CLI exists with `command -v openspec`.
   - Locate the workflow root before running commands. Prefer `docs/` when present, but detect instead of assuming.
   - If OpenSpec exists, run `openspec list --specs` and `openspec list` from that root before proposing a change.
3. Reuse an existing change when it matches the requested capability; otherwise create a new one.
   - Run `openspec change show <change-id>` on candidates before deciding.
   - If no match, run `openspec change create <new-id>` now — do not defer this to later.
4. Keep specs and code aligned.
   - Write or update `proposal.md`, `design.md`, `tasks.md`, and at least one behavioral spec delta when the repo uses OpenSpec.
   - Keep behavior in the spec delta and implementation detail in design/tasks.
   - **Do not write or modify code until the change directory exists and has at least a `proposal.md`.**
5. Refactor around explicit seams.
   - execution policy
   - context assembly
   - stores and persistence
   - cache invalidation
   - event, telemetry, or profiling sinks
6. Validate from the terminal — **do not skip this step**.
   - Run `openspec change validate <change-id> --no-interactive` when OpenSpec is in play. The change is not done until this passes.
   - Use `openspec change show <change-id> --json --deltas-only` to confirm the actual delta surface.
   - Run targeted tests, type-checks, or terminal workflows that exercise the changed seams.
7. Keep rollout boundaries visible.
   - Call out what changed, what did not, and what remains isolated behind a boundary.

## Repo Rules

- Do not assume OpenSpec exists; detect it first.
- If OpenSpec exists, run it from the directory where the repo keeps its spec workflow, often `docs/`.
- A spec change is incomplete until the validator parses at least one real delta.
- Do not call behavior "warm", "cached", or "incremental" unless the underlying runtime semantics actually support that claim.

## Parallelism

- Parallelize only independent reads, audits, and disjoint edits.
- Do not parallelize overlapping file edits, shared metadata updates, or final integration steps.
- Serialize validation, final integration, and any archive/finalize steps.

## References

Read only what you need:

- `references/openspec-workflow.md`
- `references/parallelism.md`
- `references/principles.md`
- `references/review-checklist.md`
