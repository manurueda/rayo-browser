#!/bin/bash
# Fix Conductor Launch Script v2.2.0
#
# Each fix gets its own agent-deck profile and git worktree for true parallel execution.
#
# Usage:
#   .fix/launch.sh start "BUG_SPEC"      — start a new TDD fix pipeline (auto-generates slug)
#   .fix/launch.sh status                 — check all conductors (auto-sweeps completed)
#   .fix/launch.sh status <slug>          — check one conductor (auto-cleans if complete)
#   .fix/launch.sh stop                   — stop all conductors
#   .fix/launch.sh stop <slug>            — stop one conductor
#   .fix/launch.sh reset                  — stop all + cleanup worktrees/branches
#   .fix/launch.sh sweep                  — clean up completed fixes only

set -euo pipefail

FIX_VERSION="2.2.0"
PROJECT_DIR="$(git rev-parse --show-toplevel)"
FIX_DIR="$PROJECT_DIR/.fix"
WORKTREE_BASE="$PROJECT_DIR/.worktrees"

# Generate a slug from a description
slugify() {
  echo "$1" | tr '[:upper:]' '[:lower:]' | tr -cs '[:alnum:]' '-' | sed 's/^-//;s/-$//' | cut -c1-40
}

# Clean up a completed fix: worktree, branch, session, state file
cleanup_completed_fix() {
  local slug="$1"
  local profile="fix-$slug"
  local session="fix-$slug"
  local branch="fix/$slug"
  local worktree_dir="$WORKTREE_BASE/fix-$slug"
  local state_file="$FIX_DIR/state-$slug.json"
  local cleaned=0

  # Stop and remove agent-deck sessions
  for s in $(agent-deck -p "$profile" list --json 2>/dev/null | python3 -c "
import json, sys
try:
  for s in json.load(sys.stdin): print(s['title'])
except: pass
" 2>/dev/null); do
    agent-deck -p "$profile" session stop "$s" 2>/dev/null || true
    agent-deck -p "$profile" rm "$s" 2>/dev/null || true
    cleaned=1
  done

  # Remove worktree
  if [ -d "$worktree_dir" ]; then
    cd "$PROJECT_DIR"
    git worktree remove "$worktree_dir" --force 2>/dev/null || true
    cleaned=1
  fi

  # Remove fix branch (only if already merged to main)
  if git branch --merged main 2>/dev/null | grep -q "$branch"; then
    git branch -D "$branch" 2>/dev/null || true
    cleaned=1
  fi

  # Remove any sub-branches (worker branches like fix/slug/red-*, fix/slug/break, etc.)
  for br in $(git branch --list "fix/$slug/*" 2>/dev/null | sed 's/^[* ]*//'); do
    git branch -D "$br" 2>/dev/null || true
    cleaned=1
  done

  # Remove state file
  if [ -f "$state_file" ]; then
    rm -f "$state_file"
    cleaned=1
  fi

  if [ "$cleaned" -eq 1 ]; then
    echo "  ✓ Cleaned up fix-$slug (worktree, branch, session, state)"
  fi
}

# Check if a fix branch is merged to main and its conductor is no longer running
is_fix_completed() {
  local slug="$1"
  local branch="fix/$slug"
  local profile="fix-$slug"

  # Check if branch exists and is merged to main
  if git branch --merged main 2>/dev/null | grep -q "$branch"; then
    # Check that no session is still running
    local running
    running=$(agent-deck -p "$profile" list --json 2>/dev/null | python3 -c "
import json, sys
try:
  sessions = json.load(sys.stdin)
  running = [s for s in sessions if s.get('status') in ('running', 'waiting')]
  print(len(running))
except: print('0')
" 2>/dev/null || echo "0")
    [ "$running" = "0" ]
    return $?
  fi
  return 1
}

# Sweep all completed fixes
sweep_completed_fixes() {
  local found_any=0
  cd "$PROJECT_DIR"
  for br in $(git branch --list 'fix/*' 2>/dev/null | sed 's/^[* ]*//'); do
    # Extract slug from branch name (fix/some-slug -> some-slug, fix/some-slug/sub -> some-slug)
    local slug
    slug=$(echo "$br" | sed 's|^fix/||' | cut -d'/' -f1)
    if is_fix_completed "$slug" 2>/dev/null; then
      if [ "$found_any" -eq 0 ]; then
        echo "Sweeping completed fixes..."
        found_any=1
      fi
      cleanup_completed_fix "$slug"
    fi
  done
  # Also sweep orphaned worktrees with no matching running session
  for wt in "$WORKTREE_BASE"/fix-*; do
    if [ -d "$wt" ]; then
      local wt_slug
      wt_slug=$(basename "$wt" | sed 's/^fix-//')
      if is_fix_completed "$wt_slug" 2>/dev/null; then
        if [ "$found_any" -eq 0 ]; then
          echo "Sweeping completed fixes..."
          found_any=1
        fi
        cleanup_completed_fix "$wt_slug"
      fi
    fi
  done
  if [ "$found_any" -eq 0 ]; then
    echo "No completed fixes to sweep."
  fi
}

# List all fix sessions across all fix-* profiles as JSON
all_fix_sessions_json() {
  agent-deck list --all --json 2>/dev/null | python3 -c "
import json, sys, subprocess

all_sessions = json.load(sys.stdin)
profiles = set()
for s in all_sessions:
    p = s.get('profile', '')
    if p.startswith('fix-'):
        profiles.add(p)

all_fix = []
for p in sorted(profiles):
    try:
        result = subprocess.run(['agent-deck', '-p', p, 'list', '--json'],
                              capture_output=True, text=True, timeout=5)
        if result.returncode == 0:
            all_fix.extend(json.loads(result.stdout))
    except: pass

print(json.dumps(all_fix))
" 2>/dev/null || echo "[]"
}

case "${1:-status}" in
  status)
    SLUG="${2:-}"
    echo "=== Fix Conductor v${FIX_VERSION} — Status ==="

    if [ -n "$SLUG" ]; then
      PROFILE="fix-$SLUG"
      SESSION="fix-$SLUG"
      echo "--- $SESSION (profile: $PROFILE) ---"
      agent-deck -p "$PROFILE" session output "$SESSION" -q 2>/dev/null | tail -30 || echo "Session '$SESSION' not found."
      echo ""
      STATE_FILE="$FIX_DIR/state-$SLUG.json"
      cat "$STATE_FILE" 2>/dev/null || echo "No state file for '$SLUG'."
      # Auto-cleanup if this fix is completed
      if is_fix_completed "$SLUG" 2>/dev/null; then
        echo ""
        echo "Fix is complete and merged. Cleaning up..."
        cleanup_completed_fix "$SLUG"
      fi
    else
      all_fix_sessions_json | python3 -c "
import json, sys
sessions = json.load(sys.stdin)
waiting = sum(1 for s in sessions if s.get('status') == 'waiting')
running = sum(1 for s in sessions if s.get('status') == 'running')
idle = sum(1 for s in sessions if s.get('status') == 'idle')
print(f'{waiting} waiting • {running} running • {idle} idle')
print()
for s in sessions:
  st = s.get('status', '?')
  if st in ('running', 'waiting', 'idle'):
    print(f'  {s[\"title\"]:45} {st:10} {s.get(\"path\", \"\")[:50]}')
" 2>/dev/null || echo "No fix sessions."
      echo ""
      for sf in "$FIX_DIR"/state-*.json; do
        [ -f "$sf" ] && echo "--- $(basename "$sf") ---" && cat "$sf" && echo ""
      done
      # Auto-sweep all completed fixes
      echo ""
      sweep_completed_fixes
    fi
    ;;

  stop)
    SLUG="${2:-}"

    if [ -n "$SLUG" ]; then
      PROFILE="fix-$SLUG"
      SESSION="fix-$SLUG"
      BRANCH="fix/$SLUG"
      WORKTREE_DIR="$WORKTREE_BASE/fix-$SLUG"
      STATE_FILE="$FIX_DIR/state-$SLUG.json"
      echo "=== Stopping fix-$SLUG (profile: $PROFILE) ==="
      # Stop all sessions in this profile (conductor + any workers)
      for session in $(agent-deck -p "$PROFILE" list --json 2>/dev/null | python3 -c "
import json, sys
try:
  for s in json.load(sys.stdin): print(s['title'])
except: pass
" 2>/dev/null); do
        echo "Stopping $session..."
        agent-deck -p "$PROFILE" session stop "$session" 2>/dev/null || true
        agent-deck -p "$PROFILE" rm "$session" 2>/dev/null || true
      done
      cd "$PROJECT_DIR"
      # Clean up worktree
      if [ -d "$WORKTREE_DIR" ]; then
        echo "Removing worktree: $WORKTREE_DIR"
        git worktree remove "$WORKTREE_DIR" --force 2>/dev/null || true
      fi
      # Clean up fix branch if merged to main
      if git branch --merged main 2>/dev/null | grep -q "$BRANCH"; then
        echo "Removing merged branch: $BRANCH"
        git branch -D "$BRANCH" 2>/dev/null || true
      fi
      # Clean up any sub-branches (worker branches)
      for br in $(git branch --list "fix/$SLUG/*" 2>/dev/null | sed 's/^[* ]*//'); do
        echo "Removing worker branch: $br"
        git branch -D "$br" 2>/dev/null || true
      done
      # Clean up state file
      if [ -f "$STATE_FILE" ]; then
        echo "Removing state file"
        rm -f "$STATE_FILE"
      fi
      echo "Stopped fix-$SLUG."
    else
      echo "=== Stopping All Fix Conductors ==="
      all_fix_sessions_json | python3 -c "
import json, sys
for s in json.load(sys.stdin):
  print(f'{s[\"profile\"]} {s[\"title\"]}')
" 2>/dev/null | while read -r profile title; do
        echo "Stopping $title (profile: $profile)..."
        agent-deck -p "$profile" session stop "$title" 2>/dev/null || true
        agent-deck -p "$profile" rm "$title" 2>/dev/null || true
      done
      # Clean up all fix worktrees
      cd "$PROJECT_DIR"
      for wt in "$WORKTREE_BASE"/fix-*; do
        if [ -d "$wt" ]; then
          echo "Removing worktree: $wt"
          git worktree remove "$wt" --force 2>/dev/null || true
        fi
      done
      echo "All fix conductors stopped."
    fi
    ;;

  reset)
    echo "=== Resetting Fix Conductor ==="
    all_fix_sessions_json | python3 -c "
import json, sys
for s in json.load(sys.stdin):
  print(f'{s[\"profile\"]} {s[\"title\"]}')
" 2>/dev/null | while read -r profile title; do
      echo "Stopping $title (profile: $profile)..."
      agent-deck -p "$profile" session stop "$title" 2>/dev/null || true
      agent-deck -p "$profile" rm "$title" 2>/dev/null || true
    done
    cd "$PROJECT_DIR"
    # Remove all fix worktrees
    for wt in "$WORKTREE_BASE"/fix-*; do
      if [ -d "$wt" ]; then
        echo "Removing worktree: $wt"
        git worktree remove "$wt" --force 2>/dev/null || true
      fi
    done
    # Also catch any worktrees git knows about with fix/ in the branch
    for wt in $(git worktree list --porcelain 2>/dev/null | grep "^worktree.*/fix" | sed 's/^worktree //'); do
      echo "Removing worktree: $wt"
      git worktree remove "$wt" --force 2>/dev/null || true
    done
    for br in $(git branch --list 'fix/*' 2>/dev/null | sed 's/^[* ]*//' ); do
      echo "Removing branch: $br"
      git branch -D "$br" 2>/dev/null || true
    done
    rm -f "$FIX_DIR"/state-*.json "$FIX_DIR/state.json"
    echo "Fix conductor reset."
    ;;

  start)
    SPEC="${2:-}"
    if [ -z "$SPEC" ]; then
      echo "Usage: .fix/launch.sh start 'description of what is broken'"
      echo ""
      echo "Examples:"
      echo "  .fix/launch.sh start 'Brand system doesnt update when CEO pastes new palette in chat'"
      echo "  .fix/launch.sh start 'Run marked complete even though tool call failed'"
      echo ""
      echo "You can also pass structured JSON for pre-diagnosed bugs."
      exit 1
    fi

    echo "=== Fix Conductor v${FIX_VERSION} ==="

    if ! command -v agent-deck &> /dev/null; then
      echo "ERROR: agent-deck not found."
      exit 1
    fi

    SLUG=$(slugify "$SPEC")
    PROFILE="fix-$SLUG"
    SESSION="fix-$SLUG"
    BRANCH="fix/$SLUG"
    STATE_FILE="$FIX_DIR/state-$SLUG.json"
    WORKTREE_DIR="$WORKTREE_BASE/fix-$SLUG"

    # Check if this slug already has a running session
    EXISTING=$(agent-deck -p "$PROFILE" list --json 2>/dev/null | grep -c "\"$SESSION\"" || true)

    if [ "$EXISTING" -gt 0 ]; then
      echo "Conductor '$SESSION' already running. Sending updated spec..."
      agent-deck -p "$PROFILE" session send "$SESSION" "$SPEC" --no-wait
    else
      # Init state
      cat > "$STATE_FILE" << STATEEOF
{
  "fix_name": "$SLUG",
  "branch": "$BRANCH",
  "phase": "setup",
  "bugs": [],
  "breaker_iterations": 0,
  "scenario_pass": null,
  "started_at": "$(date -u +%Y-%m-%dT%H:%M:%SZ)",
  "completed_at": null
}
STATEEOF

      # Create isolated worktree
      cd "$PROJECT_DIR"
      git fetch origin 2>/dev/null || true
      if ! git show-ref --verify --quiet "refs/heads/$BRANCH"; then
        git branch "$BRANCH" main 2>/dev/null || true
      fi
      if [ -d "$WORKTREE_DIR" ]; then
        git worktree remove "$WORKTREE_DIR" --force 2>/dev/null || true
      fi
      mkdir -p "$WORKTREE_BASE"
      git worktree add "$WORKTREE_DIR" "$BRANCH" 2>/dev/null || {
        echo "ERROR: Failed to create worktree at $WORKTREE_DIR"
        exit 1
      }

      echo "Launching conductor v${FIX_VERSION}: $SESSION (profile: $PROFILE, branch: $BRANCH)..."
      echo "  Worktree: $WORKTREE_DIR"
      CONDUCTOR_PROMPT=$(cat << PROMPTEOF
You are the Fix Conductor v${FIX_VERSION}. Read .fix/conductor-claude.md NOW for your full instructions.

Your fix slug is: $SLUG
Your session name is: $SESSION
Your profile is: $PROFILE
Your branch is: $BRANCH
Your state file is: .fix/state-$SLUG.json
Your worktree is: $WORKTREE_DIR

You are running in an ISOLATED WORKTREE at $WORKTREE_DIR on branch $BRANCH.
Do NOT run git checkout on the main repo. You are already on your fix branch.

Here is your bug spec:

$SPEC

Start by:
1. Read .fix/conductor-claude.md
2. Read CLAUDE.md and coding-standards.md
3. You are already on branch $BRANCH in your worktree — skip branch creation
4. Begin RED phase — write failing tests first
5. Execute the full TDD pipeline autonomously

Go.
PROMPTEOF
      )
      agent-deck -p "$PROFILE" launch "$WORKTREE_DIR" \
        -t "$SESSION" \
        -c "claude --allowedTools 'Bash,Read,Write,Edit,Glob,Grep,Agent'" \
        -m "$CONDUCTOR_PROMPT"
    fi

    echo ""
    echo "✓ Fix Conductor v${FIX_VERSION} — '$SESSION' is running (profile: $PROFILE)."
    echo "  Branch: $BRANCH"
    echo "  Worktree: $WORKTREE_DIR"
    echo ""
    echo "Monitor:  .fix/launch.sh status $SLUG"
    echo "Stop:     .fix/launch.sh stop $SLUG"
    ;;

  sweep)
    echo "=== Fix Conductor v${FIX_VERSION} — Sweep ==="
    sweep_completed_fixes
    ;;

  phase)
    SLUG="${2:-}"
    PHASE="${3:-}"
    if [ -z "$SLUG" ] || [ -z "$PHASE" ]; then
      echo "Usage: .fix/launch.sh phase <slug> <phase>"
      echo "Phases: setup, diagnose, challenge, red, green, adversarial, e2e, scenario, validation, merging, complete, incomplete"
      exit 1
    fi
    STATE_FILE="$FIX_DIR/state-$SLUG.json"
    if [ ! -f "$STATE_FILE" ]; then
      echo "State file not found: $STATE_FILE"
      exit 1
    fi
    # Update phase in state file
    TMP_FILE=$(mktemp)
    sed "s/\"phase\": *\"[^\"]*\"/\"phase\": \"$PHASE\"/" "$STATE_FILE" > "$TMP_FILE" && mv "$TMP_FILE" "$STATE_FILE"
    echo "✓ fix-$SLUG → $PHASE"
    ;;

  *)
    echo "Usage: $0 [start|status|stop|reset|sweep|phase]"
    echo ""
    echo "  start 'description'   — launch a new fix conductor"
    echo "  status [slug]         — check progress (all or one)"
    echo "  stop [slug]           — stop conductors (all or one)"
    echo "  reset                 — stop all + cleanup worktrees/branches"
    echo "  sweep                 — clean up completed fixes (merged branches, worktrees)"
    echo "  phase <slug> <phase>  — update conductor phase (setup, red, green, ...)"
    ;;
esac
