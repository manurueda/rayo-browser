# Binary Auto-Update

## ADDED Requirements

### Requirement: rayo-mcp checks for updates on startup without blocking

rayo-mcp SHALL spawn a background task on startup that checks GitHub Releases for a newer version. The MCP server MUST start immediately without waiting for the update check. The update check SHALL be rate-limited to once per hour using a timestamp file at `~/.rayo/last-check`.

#### Scenario: Background update check on startup
Given rayo-mcp starts and the last check was more than 1 hour ago
When the MCP server initializes
Then a background task is spawned to check GitHub Releases
And the MCP server starts accepting requests immediately without waiting

#### Scenario: Rate-limited check skip
Given rayo-mcp starts and the last check was less than 1 hour ago
When the MCP server initializes
Then the update check is skipped
And the MCP server starts normally

#### Scenario: First run with no state directory
Given rayo-mcp starts for the first time with no ~/.rayo directory
When the update check runs
Then ~/.rayo is created
And the check proceeds as if the rate limit has expired

### Requirement: Updates download and replace the binary atomically

When a newer version is detected, the updater SHALL download the platform-appropriate binary via axoupdater, verify its SHA256 checksum, back up the current binary, and atomically replace it. The update applies on the next startup, never mid-session.

#### Scenario: Successful update download and replace
Given a newer version exists on GitHub Releases
When the update check detects the newer version
Then the correct platform binary is downloaded
And the SHA256 checksum is verified against the release manifest
And the current binary is backed up to .rayo-mcp.backup
And the new binary atomically replaces the current one
And an update marker is written to ~/.rayo/update-marker.json

#### Scenario: Checksum mismatch rejects the download
Given a newer version is downloaded but the checksum does not match
When the checksum verification runs
Then the downloaded file is deleted
And the update is skipped
And the current binary remains unchanged

#### Scenario: Permission denied on binary replacement
Given the current binary path is not writable
When the updater attempts to replace the binary
Then a warning is logged with a fix suggestion
And the update is skipped
And the MCP server continues normally

### Requirement: Concurrent update attempts are serialized via file lock

The updater SHALL acquire an advisory file lock on `~/.rayo/update.lock` before downloading or replacing. If the lock is held by another instance, the update check is skipped.

#### Scenario: Second instance skips update when lock is held
Given two rayo-mcp instances start simultaneously
When both attempt to acquire the update lock
Then one acquires the lock and proceeds with the update check
And the other skips the update check entirely

### Requirement: Post-update startup detects and logs the update

On startup, rayo-mcp SHALL check for an update marker file. If present and the startup succeeds, it logs the version transition and clears the marker.

#### Scenario: Successful post-update startup
Given rayo-mcp starts after a successful binary replacement
When the update marker is detected
Then "rayo-mcp v{new} (updated from v{old})" is logged at info level
And the update marker is cleared

### Requirement: Failed update triggers rollback from backup

If rayo-mcp starts after an update but crashes before clearing the marker, the next startup SHALL detect the stale marker and restore the backup binary. After 2 consecutive crash-rollback cycles, auto-update is disabled with an error log.

#### Scenario: Rollback after crash
Given rayo-mcp was updated but crashed on the first post-update startup
When rayo-mcp starts again and finds a stale update marker
Then the backup binary is restored to the current binary path
And a warning is logged indicating rollback occurred

#### Scenario: Crash loop disables auto-update
Given rayo-mcp has crashed after update and been rolled back twice consecutively
When rayo-mcp starts again
Then auto-update is disabled
And an error is logged with instructions to manually update or file a bug

### Requirement: Auto-update can be disabled via environment variable

Setting `RAYO_NO_UPDATE=1` SHALL disable all update checking and replacement. The MCP server starts normally with no update-related network calls.

#### Scenario: RAYO_NO_UPDATE disables updates
Given RAYO_NO_UPDATE=1 is set in the environment
When rayo-mcp starts
Then no update check is performed
And no network calls to GitHub are made
And the MCP server starts normally

### Requirement: All update errors are non-fatal

Every error in the update pipeline (network, filesystem, checksum, permissions) SHALL be caught and logged at debug or warn level. No update error SHALL prevent the MCP server from starting.

#### Scenario: No internet does not block startup
Given rayo-mcp starts with no network connectivity
When the background update check fails to reach GitHub
Then the failure is logged at debug level
And the MCP server continues normally
