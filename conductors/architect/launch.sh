#!/bin/bash
# Architect Conductor Launch Script
#
# Usage:
#   .architect/launch.sh              — full architecture review
#   .architect/launch.sh status       — check progress
#   .architect/launch.sh stop         — stop everything
#   .architect/launch.sh reset        — stop + cleanup
#   .architect/launch.sh collect      — run data collection only
#   .architect/launch.sh query "SQL"  — query the database

set -euo pipefail

PROFILE="architect"
PROJECT_DIR="$(git rev-parse --show-toplevel)"
ARCHITECT_DIR="$PROJECT_DIR/.architect"
DB="$ARCHITECT_DIR/architect.db"

case "${1:-start}" in
  status)
    echo "=== Architect Status ==="
    agent-deck -p "$PROFILE" status 2>/dev/null || echo "No architect sessions found."
    echo ""
    echo "--- State ---"
    cat "$ARCHITECT_DIR/state.json" 2>/dev/null || echo "No state file."
    echo ""
    echo "--- Database ---"
    if [ -f "$DB" ]; then
      sqlite3 "$DB" "SELECT key, value FROM meta;" 2>/dev/null || true
      echo ""
      sqlite3 "$DB" "SELECT * FROM v_region_summary;" 2>/dev/null || true
    else
      echo "No database. Run: .architect/launch.sh collect"
    fi
    echo ""
    echo "--- Artifacts ---"
    for f in CENSUS.md AUDIT.md PLAN.md; do
      if [ -f "$ARCHITECT_DIR/$f" ]; then
        echo "  $f: $(wc -l < "$ARCHITECT_DIR/$f") lines"
      else
        echo "  $f: not yet generated"
      fi
    done
    exit 0
    ;;

  stop)
    echo "=== Stopping Architect ==="
    for session in $(agent-deck -p "$PROFILE" list --json 2>/dev/null | grep -o '"title":"[^"]*"' | cut -d'"' -f4); do
      echo "Stopping $session..."
      agent-deck -p "$PROFILE" session stop "$session" 2>/dev/null || true
    done
    echo "Architect stopped."
    exit 0
    ;;

  reset)
    echo "=== Resetting Architect ==="
    for session in $(agent-deck -p "$PROFILE" list --json 2>/dev/null | grep -o '"title":"[^"]*"' | cut -d'"' -f4); do
      agent-deck -p "$PROFILE" session stop "$session" 2>/dev/null || true
    done
    rm -f "$ARCHITECT_DIR/state.json"
    rm -f "$ARCHITECT_DIR/architect.db"
    rm -f "$ARCHITECT_DIR/CENSUS.md"
    rm -f "$ARCHITECT_DIR/AUDIT.md"
    rm -f "$ARCHITECT_DIR/PLAN.md"
    echo "Architect reset complete."
    exit 0
    ;;

  collect)
    echo "=== Running Data Collection ==="
    "$ARCHITECT_DIR/collect.sh"
    exit 0
    ;;

  query)
    if [ -z "${2:-}" ]; then
      echo "Usage: .architect/launch.sh query \"SELECT * FROM v_region_summary;\""
      exit 1
    fi
    sqlite3 -header -column "$DB" "$2"
    exit 0
    ;;

  start)
    ;;

  *)
    echo "Usage: $0 [start|status|stop|reset|collect|query \"SQL\"]"
    exit 1
    ;;
esac

# --- Main Launch ---

echo "=== Architect Launch ==="
echo "Profile: $PROFILE"
echo "Project: $PROJECT_DIR"
echo ""

if ! command -v agent-deck &> /dev/null; then
    echo "ERROR: agent-deck not found."
    exit 1
fi

cd "$PROJECT_DIR"
# Check for modified/staged files only (ignore untracked like architect.db)
DIRTY=$(git status --porcelain | grep -v '^??' || true)
if [ -n "$DIRTY" ]; then
    echo "WARNING: main has uncommitted changes:"
    echo "$DIRTY"
    echo ""
    if [ -t 0 ]; then
        read -rp "Continue anyway? (y/N): " confirm
        if [ "$confirm" != "y" ] && [ "$confirm" != "Y" ]; then
            echo "Aborted. Commit or stash first."
            exit 1
        fi
    else
        echo "Running non-interactively — proceeding despite dirty state."
    fi
fi

# Step 1: Run data collection
echo "--- Step 1: Data Collection ---"
"$ARCHITECT_DIR/collect.sh"
echo ""

# Step 2: Initialize state
if [ ! -f "$ARCHITECT_DIR/state.json" ]; then
    cat > "$ARCHITECT_DIR/state.json" << 'STATEOF'
{
  "run_counter": 0,
  "last_run": null,
  "current_phase": null,
  "current_worker": null,
  "phases_completed": [],
  "history": []
}
STATEOF
    echo "Created state.json"
fi

# Step 3: Clear previous artifacts
rm -f "$ARCHITECT_DIR/CENSUS.md"
rm -f "$ARCHITECT_DIR/AUDIT.md"
rm -f "$ARCHITECT_DIR/PLAN.md"

# Step 4: Launch conductor
echo ""
echo "--- Step 2: Launching Conductor ---"

EXISTING=$(agent-deck -p "$PROFILE" list --json 2>/dev/null | grep -c "architect-conductor" || true)

if [ "$EXISTING" -gt 0 ]; then
    echo "Conductor exists. Restarting..."
    agent-deck -p "$PROFILE" session restart architect-conductor
else
    CONDUCTOR_PROMPT=$(cat << 'PROMPTEOF'
You are the Architect Conductor. Read .architect/conductor-claude.md NOW for your full instructions.

The database has already been populated by collect.sh. Skip step 2 (collection) and go directly to:
1. Read .architect/conductor-claude.md
2. Read .architect/state.json
3. Verify: sqlite3 .architect/architect.db "SELECT * FROM v_region_summary;"
4. Launch the surveyor

Execute: survey → audit → prescribe → present.
PROMPTEOF
    )
    agent-deck -p "$PROFILE" launch "$PROJECT_DIR" \
        -t "architect-conductor" \
        -c "claude --allowedTools 'Bash,Read,Write,Edit,Glob,Grep,Agent'" \
        -m "$CONDUCTOR_PROMPT"
fi

echo ""
echo "Architect conductor is running."
echo ""
echo "Commands:"
echo "  .architect/launch.sh status    — check progress"
echo "  .architect/launch.sh stop      — stop"
echo "  .architect/launch.sh reset     — stop + cleanup"
echo "  .architect/launch.sh query SQL — query the database"
echo ""
echo "Monitor:"
echo "  agent-deck -p $PROFILE session output architect-conductor -q"
echo ""
echo "Artifacts:"
echo "  .architect/architect.db — queryable database"
echo "  .architect/CENSUS.md   — raw codebase data"
echo "  .architect/AUDIT.md    — architectural diagnosis"
echo "  .architect/PLAN.md     — transformation proposals"
