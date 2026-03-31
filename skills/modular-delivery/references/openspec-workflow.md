# OpenSpec Workflow

Use this only when the repository actually uses OpenSpec.

## Find the working directory

Identify the directory that owns the OpenSpec workflow before running commands. In many repos this is `docs/`, but do not assume that blindly.

Typical commands:

```bash
openspec list --specs
openspec list
openspec change validate <change-id> --no-interactive
openspec change show <change-id> --json --deltas-only
```

## Requirements for a valid change

- `proposal.md`
- `design.md`
- `tasks.md`
- at least one `specs/<capability>/spec.md`

## Delta checklist

1. Use `## ADDED Requirements` or `## MODIFIED Requirements`.
2. Add one or more `### Requirement:`.
3. Every requirement needs at least one `#### Scenario:`.
4. Keep each requirement behavioral, not implementation-specific.

## Delivery rule

If the user asked for research only, stop at a validated change and a test plan.
If the user asked for implementation, keep code changes aligned with the declared capability boundaries.
