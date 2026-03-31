#!/bin/bash
# Feature Conductor Launch Script
#
# Each feature gets its own agent-deck profile (feature-SLUG) for parallel execution.
#
# Usage:
#   .feature/launch.sh start "SPEC"       — start a new feature pipeline (auto-generates slug)
#   .feature/launch.sh status             — check all feature conductors
#   .feature/launch.sh status <slug>      — check one feature conductor
#   .feature/launch.sh stop               — stop all feature conductors
#   .feature/launch.sh stop <slug>        — stop one feature conductor
#   .feature/launch.sh reset             — stop all + cleanup worktrees/branches

set -euo pipefail

FEATURE_VERSION="2.1.0"
PROJECT_DIR="$(git rev-parse --show-toplevel)"
FEATURE_DIR="$PROJECT_DIR/.feature"
WORKTREE_BASE="$PROJECT_DIR/.worktrees"

slugify() {
  echo "$1" | tr '[:upper:]' '[:lower:]' | tr -cs '[:alnum:]' '-' | sed 's/^-//;s/-$//' | cut -c1-40
}

all_feature_sessions_json() {
  agent-deck list --all --json 2>/dev/null | python3 -c "
import json, sys, subprocess

all_sessions = json.load(sys.stdin)
profiles = set()
for s in all_sessions:
    p = s.get('profile', '')
    if p == 'feature' or p.startswith('feature-'):
        profiles.add(p)

all_feat = []
for p in sorted(profiles):
    try:
        result = subprocess.run(['agent-deck', '-p', p, 'list', '--json'],
                              capture_output=True, text=True, timeout=5)
        if result.returncode == 0:
            all_feat.extend(json.loads(result.stdout))
    except: pass

print(json.dumps(all_feat))
" 2>/dev/null || echo "[]"
}

case "${1:-status}" in
  status)
    SLUG="${2:-}"
    echo "=== Feature Conductor v${FEATURE_VERSION} — Status ==="

    if [ -n "$SLUG" ]; then
      PROFILE="feature-$SLUG"
      SESSION="feat-$SLUG"
      echo "--- $SESSION (profile: $PROFILE) ---"
      agent-deck -p "$PROFILE" session output "$SESSION" -q 2>/dev/null | tail -30 || echo "Session '$SESSION' not found."
      echo ""
      STATE_FILE="$FEATURE_DIR/state-$SLUG.json"
      cat "$STATE_FILE" 2>/dev/null || echo "No state file for '$SLUG'."
    else
      all_feature_sessions_json | python3 -c "
import json, sys
sessions = json.load(sys.stdin)
waiting = sum(1 for s in sessions if s['status'] == 'waiting')
running = sum(1 for s in sessions if s['status'] == 'running')
idle = sum(1 for s in sessions if s['status'] == 'idle')
print(f'{waiting} waiting • {running} running • {idle} idle')
print()
for s in sessions:
  if s['status'] in ('running', 'waiting', 'idle'):
    print(f'  {s[\"title\"]:45} {s[\"status\"]:10} {s.get(\"profile\", \"\"):25} {s[\"path\"][:40]}')
" 2>/dev/null || echo "No feature sessions."
      echo ""
      for sf in "$FEATURE_DIR"/state-*.json; do
        [ -f "$sf" ] && echo "--- $(basename "$sf") ---" && cat "$sf" && echo ""
      done
    fi
    ;;

  stop)
    SLUG="${2:-}"

    if [ -n "$SLUG" ]; then
      PROFILE="feature-$SLUG"
      WORKTREE_DIR="$WORKTREE_BASE/feat-$SLUG"
      echo "=== Stopping feat-$SLUG (profile: $PROFILE) ==="
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
      if [ -d "$WORKTREE_DIR" ]; then
        echo "Removing worktree: $WORKTREE_DIR"
        cd "$PROJECT_DIR" && git worktree remove "$WORKTREE_DIR" --force 2>/dev/null || true
      fi
      echo "Stopped feat-$SLUG."
    else
      echo "=== Stopping All Feature Conductors ==="
      all_feature_sessions_json | python3 -c "
import json, sys
for s in json.load(sys.stdin):
  print(f'{s[\"profile\"]} {s[\"title\"]}')
" 2>/dev/null | while read -r profile title; do
        echo "Stopping $title (profile: $profile)..."
        agent-deck -p "$profile" session stop "$title" 2>/dev/null || true
        agent-deck -p "$profile" rm "$title" 2>/dev/null || true
      done
      cd "$PROJECT_DIR"
      for wt in "$WORKTREE_BASE"/feat-*; do
        [ -d "$wt" ] && echo "Removing worktree: $wt" && git worktree remove "$wt" --force 2>/dev/null || true
      done
      echo "All feature conductors stopped."
    fi
    ;;

  reset)
    echo "=== Resetting Feature Conductor ==="
    all_feature_sessions_json | python3 -c "
import json, sys
for s in json.load(sys.stdin):
  print(f'{s[\"profile\"]} {s[\"title\"]}')
" 2>/dev/null | while read -r profile title; do
      echo "Stopping $title (profile: $profile)..."
      agent-deck -p "$profile" session stop "$title" 2>/dev/null || true
      agent-deck -p "$profile" rm "$title" 2>/dev/null || true
    done
    cd "$PROJECT_DIR"
    for wt in "$WORKTREE_BASE"/feat-*; do
      [ -d "$wt" ] && echo "Removing worktree: $wt" && git worktree remove "$wt" --force 2>/dev/null || true
    done
    for wt in $(git worktree list --porcelain 2>/dev/null | grep "^worktree.*feature/" | sed 's/^worktree //'); do
      echo "Removing worktree: $wt"
      git worktree remove "$wt" --force 2>/dev/null || true
    done
    for br in $(git branch --list 'feature/*' 2>/dev/null | sed 's/^[* ]*//' ); do
      echo "Removing branch: $br"
      git branch -D "$br" 2>/dev/null || true
    done
    rm -f "$FEATURE_DIR"/state-*.json "$FEATURE_DIR/state.json"
    echo "Feature conductor reset."
    ;;

  start)
    SPEC="${2:-}"
    if [ -z "$SPEC" ]; then
      echo "Usage: .feature/launch.sh start 'description of the feature'"
      echo ""
      echo "Examples:"
      echo "  .feature/launch.sh start 'Add dark mode toggle to settings page'"
      echo "  .feature/launch.sh start 'LinkedIn image and carousel support'"
      echo ""
      echo "You can also pass structured JSON with modules and acceptance criteria."
      exit 1
    fi

    echo "=== Feature Conductor v${FEATURE_VERSION} ==="

    if ! command -v agent-deck &> /dev/null; then
      echo "ERROR: agent-deck not found."
      exit 1
    fi

    SLUG=$(slugify "$SPEC")
    PROFILE="feature-$SLUG"
    SESSION="feat-$SLUG"
    BRANCH="feature/$SLUG"
    STATE_FILE="$FEATURE_DIR/state-$SLUG.json"
    WORKTREE_DIR="$WORKTREE_BASE/feat-$SLUG"

    EXISTING=$(agent-deck -p "$PROFILE" list --json 2>/dev/null | grep -c "\"$SESSION\"" || true)

    if [ "$EXISTING" -gt 0 ]; then
      echo "Conductor '$SESSION' already running. Sending updated spec..."
      agent-deck -p "$PROFILE" session send "$SESSION" "$SPEC" --no-wait
    else
      cat > "$STATE_FILE" << STATEEOF
{
  "feature_name": "$SLUG",
  "branch": "$BRANCH",
  "phase": "setup",
  "modules": [],
  "breaker_iterations": 0,
  "scenario_pass": null,
  "ui_test_pass": null,
  "merged": false,
  "pushed": false,
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

      echo "Launching conductor v${FEATURE_VERSION}: $SESSION (profile: $PROFILE, branch: $BRANCH)..."
      echo "  Worktree: $WORKTREE_DIR"
      CONDUCTOR_PROMPT=$(cat << PROMPTEOF
You are the Feature Conductor v${FEATURE_VERSION}. Read .feature/conductor-claude.md NOW for your full instructions.

Your feature slug is: $SLUG
Your session name is: $SESSION
Your profile is: $PROFILE
Your branch is: $BRANCH
Your state file is: .feature/state-$SLUG.json
Your worktree is: $WORKTREE_DIR

You are running in an ISOLATED WORKTREE at $WORKTREE_DIR on branch $BRANCH.
Do NOT run git checkout on the main repo. You are already on your feature branch.

Here is your feature spec:

$SPEC

Start by:
1. Read .feature/conductor-claude.md
2. Read CLAUDE.md and coding-standards.md
3. You are already on branch $BRANCH in your worktree — skip branch creation
4. Launch the spec writer
5. Execute the full pipeline autonomously

Go.
PROMPTEOF
      )
      agent-deck -p "$PROFILE" launch "$WORKTREE_DIR" \
        -t "$SESSION" \
        -c "claude --allowedTools 'Bash,Read,Write,Edit,Glob,Grep,Agent'" \
        -m "$CONDUCTOR_PROMPT"
    fi

    echo ""
    echo "✓ Feature Conductor v${FEATURE_VERSION} — '$SESSION' is running (profile: $PROFILE)."
    echo "  Branch: $BRANCH"
    echo "  Worktree: $WORKTREE_DIR"
    echo ""
    echo "Monitor:  .feature/launch.sh status $SLUG"
    echo "Stop:     .feature/launch.sh stop $SLUG"
    ;;

  *)
    echo "Usage: $0 [start|status|stop|reset]"
    echo ""
    echo "  start 'description'   — launch a new feature conductor"
    echo "  status [slug]         — check progress (all or one)"
    echo "  stop [slug]           — stop conductors (all or one)"
    echo "  reset                 — stop all + cleanup worktrees/branches"
    ;;
esac
