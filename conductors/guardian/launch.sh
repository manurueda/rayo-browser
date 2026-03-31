#!/bin/bash
# Guardian Launch Script
# Bootstraps the guardian conductor which then manages scanner + cleaner workers autonomously.
#
# Usage:
#   .guardian/launch.sh          # start the guardian
#   .guardian/launch.sh status   # check guardian status
#   .guardian/launch.sh stop     # stop everything
#   .guardian/launch.sh reset    # stop + clean up all worktrees/branches/state

set -euo pipefail

PROFILE="guardian"
PROJECT_DIR="$(git rev-parse --show-toplevel)"
GUARDIAN_DIR="$PROJECT_DIR/.guardian"

# --- Subcommands ---

case "${1:-start}" in
  status)
    echo "=== Guardian Status ==="
    agent-deck -p "$PROFILE" status 2>/dev/null || echo "No guardian sessions found."
    echo ""
    echo "--- Work Queue ---"
    head -10 "$GUARDIAN_DIR/WORK_QUEUE.md" 2>/dev/null || echo "No work queue."
    echo ""
    echo "--- State ---"
    cat "$GUARDIAN_DIR/state.json" 2>/dev/null || echo "No state file."
    exit 0
    ;;

  stop)
    echo "=== Stopping Guardian ==="
    # Stop all sessions in the guardian profile
    for session in $(agent-deck -p "$PROFILE" list --json 2>/dev/null | grep -o '"title":"[^"]*"' | cut -d'"' -f4); do
      echo "Stopping $session..."
      agent-deck -p "$PROFILE" session stop "$session" 2>/dev/null || true
    done
    echo "Guardian stopped."
    exit 0
    ;;

  reset)
    echo "=== Resetting Guardian ==="
    # Stop all sessions
    for session in $(agent-deck -p "$PROFILE" list --json 2>/dev/null | grep -o '"title":"[^"]*"' | cut -d'"' -f4); do
      echo "Stopping $session..."
      agent-deck -p "$PROFILE" session stop "$session" 2>/dev/null || true
    done
    # Clean up all guardian worktrees
    cd "$PROJECT_DIR"
    for wt in $(git worktree list --porcelain 2>/dev/null | grep "^worktree.*guardian" | sed 's/^worktree //'); do
      echo "Removing worktree: $wt"
      git worktree remove "$wt" --force 2>/dev/null || true
    done
    # Clean up guardian branches
    for branch in $(git branch --list 'guardian/*' 2>/dev/null | tr -d ' *'); do
      echo "Deleting branch: $branch"
      git branch -D "$branch" 2>/dev/null || true
    done
    # Reset state
    rm -f "$GUARDIAN_DIR/state.json"
    echo "Guardian reset complete."
    exit 0
    ;;

  start)
    # Fall through to main launch logic
    ;;

  *)
    echo "Usage: $0 [start|status|stop|reset]"
    exit 1
    ;;
esac

# --- Main Launch ---

echo "=== Guardian Launch ==="
echo "Profile: $PROFILE"
echo "Project: $PROJECT_DIR"
echo ""

# Check agent-deck is installed
if ! command -v agent-deck &> /dev/null; then
    echo "ERROR: agent-deck not found. Install it first."
    exit 1
fi

# Check main is clean
cd "$PROJECT_DIR"
DIRTY=$(git status --porcelain | grep -v '^??' || true)
if [ -n "$DIRTY" ]; then
    echo "WARNING: main has uncommitted changes:"
    echo "$DIRTY"
    echo ""
    if [ -t 0 ]; then
        read -rp "Continue anyway? (y/N): " confirm
        if [ "$confirm" != "y" ] && [ "$confirm" != "Y" ]; then
            echo "Aborted. Commit or stash your changes first."
            exit 1
        fi
    else
        echo "Running non-interactively — proceeding despite dirty state."
    fi
fi

# Clean up any orphaned guardian worktrees from previous runs
ORPHANED_WTS=$(git worktree list --porcelain 2>/dev/null | grep "^worktree.*guardian" | sed 's/^worktree //' || true)
if [ -n "$ORPHANED_WTS" ]; then
    echo "Cleaning up orphaned worktrees from previous run..."
    while IFS= read -r wt; do
        git worktree remove "$wt" --force 2>/dev/null || true
        echo "  Removed: $wt"
    done <<< "$ORPHANED_WTS"
    # Clean orphaned branches too
    for branch in $(git branch --list 'guardian/*' 2>/dev/null | tr -d ' *'); do
        git branch -D "$branch" 2>/dev/null || true
        echo "  Deleted branch: $branch"
    done
fi

# Initialize state file
if [ ! -f "$GUARDIAN_DIR/state.json" ]; then
    cat > "$GUARDIAN_DIR/state.json" << 'STATEOF'
{
  "cleaner_counter": 0,
  "scanner_runs": 0,
  "tasks_completed": 0,
  "tasks_skipped": 0,
  "tasks_since_last_scan": 0,
  "current_task": null,
  "current_cleaner": null,
  "current_cleaner_launched_at": null,
  "history": []
}
STATEOF
    echo "Created state.json"
fi

# Check if conductor already exists in this profile
EXISTING=$(agent-deck -p "$PROFILE" list --json 2>/dev/null | grep -c "guardian-conductor" || true)

if [ "$EXISTING" -gt 0 ]; then
    echo "Conductor session already exists. Restarting..."
    agent-deck -p "$PROFILE" session restart guardian-conductor
else
    echo "Launching conductor..."
    CONDUCTOR_PROMPT=$(cat << 'PROMPTEOF'
You are the Guardian Conductor. Read your full instructions at .guardian/conductor-claude.md NOW and follow them exactly.

Start by:
1. Read .guardian/conductor-claude.md (your complete instructions)
2. Read .guardian/state.json (restore state)
3. Read .guardian/WORK_QUEUE.md (current task queue)
4. Run: git status --porcelain (verify main is clean)
5. Run: agent-deck -p guardian status --json (check for orphaned sessions)
6. Run: git worktree list (check for orphaned worktrees)
7. Clean up any orphans from previous runs
8. Launch the scanner as your first action

This is fully autonomous. Do not ask for confirmation. Execute the loop until the codebase is clean.
PROMPTEOF
    )
    agent-deck -p "$PROFILE" launch "$PROJECT_DIR" \
        -t "guardian-conductor" \
        -c "claude --allowedTools 'Bash,Read,Write,Edit,Glob,Grep,Agent'" \
        -m "$CONDUCTOR_PROMPT"
fi

echo ""
echo "Guardian conductor is running autonomously."
echo ""
echo "Commands:"
echo "  .guardian/launch.sh status   — check progress"
echo "  .guardian/launch.sh stop     — stop the guardian"
echo "  .guardian/launch.sh reset    — stop + full cleanup"
echo ""
echo "Monitor:"
echo "  agent-deck -p $PROFILE session output guardian-conductor -q"
