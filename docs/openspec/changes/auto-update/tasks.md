# Auto-Update: Tasks

## Phase 1: Release Infrastructure
- [x] Run `cargo dist init` to generate release workflow and installer scripts
- [x] Configure platform targets: aarch64-apple-darwin, x86_64-apple-darwin, x86_64-unknown-linux-gnu
- [ ] Verify CI generates release artifacts on tag push (requires first `git tag v0.1.0 && git push --tags`)
- [x] Update README install instructions to use cargo-dist installer script

## Phase 2: rayo-updater Crate
- [x] Create `crates/rayo-updater/` with Cargo.toml (deps: axoupdater, semver, serde, serde_json, tracing, fs2)
- [x] Implement `state.rs`: state dir init, last-check read/write, update marker read/write/clear, file lock acquire/release
- [x] Implement `config.rs`: UpdateConfig with env var support (RAYO_NO_UPDATE, RAYO_UPDATE_INTERVAL_SECS)
- [x] Implement `lib.rs`: public API (`check_and_update`, `handle_startup_marker`, `UpdateConfig::from_env`)

## Phase 3: Integration
- [x] Wire updater into `rayo-mcp/src/main.rs`: marker check (sync), background update (async spawn)
- [x] Add `RAYO_NO_UPDATE=1` env var support in UpdateConfig::from_env()
- [x] Add version info to `rayo_profile` output (appended to ai_summary and markdown formats)
- [x] Add `--version` / `-V` CLI flag

## Phase 4: Tests
- [x] Unit tests: state file read/write, rate limiting, config struct (16 tests passing)
- [x] Unit tests: crash loop detection, marker lifecycle, rollback logic
- [x] Integration tests: file lock contention (two tasks, second skips)
- [x] Integration tests: check_and_update skips when disabled/rate-limited
- [ ] CI-only test: real GitHub API check against published release
- [ ] Integration tests: full end-to-end update flow with real binary replacement
- [ ] Chaos tests: kill mid-download, kill mid-replace, verify no corruption

## Phase 5: Documentation
- [x] Update CLAUDE.md: add rayo-updater to crate list, architecture diagram, install instructions, conventions
- [x] Update README: cargo-dist installer, auto-update mention, RAYO_NO_UPDATE docs
- [x] Update TODOS.md: add Phase 2/3 items (channels, telemetry, generic crate extraction)
