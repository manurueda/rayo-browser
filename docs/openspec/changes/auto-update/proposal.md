# Auto-Update: Self-updating binary via cargo-dist + axoupdater

## Why
rayo is an MCP tool that runs invisibly — users install it once and forget. Without auto-update, users run stale versions for months, file bugs that are already fixed, and miss performance improvements. The difference between "tells you to update" and "updates itself" is the difference between 5% and 95% adoption of new versions.

## Solution
New `rayo-updater` crate that checks GitHub Releases on startup (background, non-blocking), downloads pre-built platform binaries via axoupdater, atomically replaces the current binary, and rolls back if the new version crashes. cargo-dist handles the release pipeline (build, sign, publish).

## Architecture

```
  rayo-mcp startup
       │
       ├──▶ Start MCP server (immediate, not blocked)
       │
       └──▶ tokio::spawn(updater::check_and_update())
                │
                ├── Read ~/.rayo/last-check (rate limit: 1/hour)
                ├── GET GitHub Releases API (latest version)
                ├── Compare semver (current vs latest)
                ├── Download platform binary via axoupdater
                ├── Verify SHA256 checksum
                ├── Acquire file lock (~/.rayo/update.lock)
                ├── Backup current binary → .rayo-mcp.backup
                ├── Atomic rename: new binary → current path
                └── Write update marker (~/.rayo/update-marker.json)

  Next startup:
       ├── Detect marker → log "Updated from v0.1.0 → v0.2.0"
       ├── If crash (stale marker) → rollback from backup
       └── If 2 consecutive crashes → disable auto-update, log error
```

## Dependency Graph

```
  rayo-profiler
       ↑
  rayo-rules    rayo-updater (NEW — no rayo deps)
       ↑              ↑
  rayo-core           │
       ↑              │
  rayo-mcp ───────────┘
```

## Scope

### New crate: rayo-updater
- `lib.rs` — Public API: `check_and_update()`, `UpdateConfig`
- `checker.rs` — Version check against GitHub Releases API
- `downloader.rs` — Download binary + SHA256 verification
- `replacer.rs` — Atomic binary replacement + backup
- `rollback.rs` — Crash detection + rollback from backup
- `state.rs` — State files in `~/.rayo/` (last-check, marker, lock)

### rayo-mcp
- `main.rs` — Spawn background update task on startup
- `main.rs` — Detect update marker, log post-update message
- `server.rs` — Add version/up-to-date to `rayo_profile` output

### Repo infrastructure
- `cargo-dist` init — release workflow, installer scripts, platform binaries
- README — Update install instructions to cargo-dist installer
- CLAUDE.md — Add rayo-updater to crate list
- TODOS.md — Add Phase 2/3 items (channels, telemetry)

### Config / escape hatches
- `RAYO_NO_UPDATE=1` env var to disable
- Rate limit: 1 check per hour (stored in `~/.rayo/last-check`)
- 5s timeout on GitHub API, 30s on download

### Boundary
- Update is ALWAYS background — never blocks MCP server startup
- Update applies on NEXT startup — never mid-session
- Every error is non-fatal — skip update, continue normally
- No new rayo-core changes — updater is fully isolated
- macOS (arm64, x86_64) + Linux (x86_64) only — no Windows
