#!/bin/bash
# Conductor & Skills Installer
#
# Installs conductor directories into the current project and
# skills into ~/.claude/skills/.
#
# Usage:
#   conductors/install.sh          — install everything
#   conductors/install.sh --dry    — show what would be done

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"
PROJECT_ROOT="$(git rev-parse --show-toplevel 2>/dev/null || pwd)"
SKILLS_DIR="$HOME/.claude/skills"
VERSION=$(cat "$SCRIPT_DIR/VERSION" 2>/dev/null || echo "unknown")
DRY_RUN=false

if [ "${1:-}" = "--dry" ]; then
  DRY_RUN=true
  echo "=== DRY RUN ==="
  echo ""
fi

echo "=== Conductor & Skills Installer v${VERSION} ==="
echo "Source:       $REPO_ROOT"
echo "Project root: $PROJECT_ROOT"
echo "Skills dir:   $SKILLS_DIR"
echo ""

# --- Helper ---
do_copy() {
  local src="$1" dst="$2"
  if $DRY_RUN; then
    echo "  [copy] $src -> $dst"
  else
    mkdir -p "$(dirname "$dst")"
    cp -R "$src" "$dst"
  fi
}

# --- Step 1: Copy conductor directories into project root ---
echo "--- Installing conductors ---"

for conductor in fix feature guardian architect; do
  SRC="$SCRIPT_DIR/$conductor"
  DST="$PROJECT_ROOT/.$conductor"

  if [ ! -d "$SRC" ]; then
    echo "  WARN: Source not found: $SRC — skipping."
    continue
  fi

  if $DRY_RUN; then
    echo "  [mkdir] $DST"
  else
    mkdir -p "$DST"
  fi

  for file in "$SRC"/*; do
    [ -f "$file" ] || continue
    BASENAME="$(basename "$file")"
    do_copy "$file" "$DST/$BASENAME"
  done
  echo "  .$conductor/ installed."
done

# --- Step 2: Copy skills to ~/.claude/skills/ ---
echo ""
echo "--- Installing skills ---"

SKILLS_SRC="$REPO_ROOT/skills"
if [ -d "$SKILLS_SRC" ]; then
  for skill_dir in "$SKILLS_SRC"/*/; do
    [ -d "$skill_dir" ] || continue
    SKILL_NAME="$(basename "$skill_dir")"
    DST_SKILL="$SKILLS_DIR/$SKILL_NAME"

    if $DRY_RUN; then
      echo "  [mkdir] $DST_SKILL"
    else
      mkdir -p "$DST_SKILL"
    fi

    # Copy all files recursively
    if $DRY_RUN; then
      find "$skill_dir" -type f | while read -r f; do
        REL="${f#$skill_dir}"
        echo "  [copy] $f -> $DST_SKILL/$REL"
      done
    else
      cp -R "$skill_dir"/* "$DST_SKILL/"
    fi
    echo "  $SKILL_NAME/ installed."
  done
else
  echo "  WARN: No skills directory found at $SKILLS_SRC"
fi

# --- Step 3: Make launch.sh files executable ---
echo ""
echo "--- Setting permissions ---"

for launcher in "$PROJECT_ROOT"/.fix/launch.sh \
                "$PROJECT_ROOT"/.feature/launch.sh \
                "$PROJECT_ROOT"/.guardian/launch.sh \
                "$PROJECT_ROOT"/.guardian/mark-complete.sh \
                "$PROJECT_ROOT"/.architect/launch.sh \
                "$PROJECT_ROOT"/.architect/collect.sh; do
  if [ -f "$launcher" ]; then
    if $DRY_RUN; then
      echo "  [chmod +x] $launcher"
    else
      chmod +x "$launcher"
    fi
  fi
done
echo "  Done."

# --- Step 4: Create initial templates ---
echo ""
echo "--- Creating templates ---"

WORK_QUEUE="$PROJECT_ROOT/.guardian/WORK_QUEUE.md"
if [ ! -f "$WORK_QUEUE" ] || $DRY_RUN; then
  if $DRY_RUN; then
    echo "  [create] $WORK_QUEUE"
  else
    mkdir -p "$PROJECT_ROOT/.guardian"
    cat > "$WORK_QUEUE" << 'EOF'
# Guardian Work Queue

## Status
- Completed: 0
- Pending: 0
- Skipped: 0

## Tasks

<!-- Add tasks as: - [ ] Description -->
EOF
    echo "  Created WORK_QUEUE.md"
  fi
else
  echo "  WORK_QUEUE.md already exists — skipping."
fi

BUG_REPORT="$PROJECT_ROOT/.guardian/BUG_REPORT.md"
if [ ! -f "$BUG_REPORT" ] || $DRY_RUN; then
  if $DRY_RUN; then
    echo "  [create] $BUG_REPORT"
  else
    mkdir -p "$PROJECT_ROOT/.guardian"
    cat > "$BUG_REPORT" << 'EOF'
# Bug Report

Bugs discovered by the guardian's bug-hunter worker.

## Open Bugs

<!-- Format:
### BUG-001: Title
- **Severity**: high/medium/low
- **File**: path/to/file.ts
- **Description**: ...
- **Discovered**: YYYY-MM-DD
-->

## Resolved Bugs

<!-- Resolved bugs are moved here -->
EOF
    echo "  Created BUG_REPORT.md"
  fi
else
  echo "  BUG_REPORT.md already exists — skipping."
fi

# --- Step 5: Update .gitignore ---
echo ""
echo "--- Updating .gitignore ---"

GITIGNORE="$PROJECT_ROOT/.gitignore"
ENTRIES=(
  ".guardian/logs/"
  ".guardian/state.json"
  ".architect/architect.db"
  ".architect/depcruiser-*.json"
  ".architect/state.json"
  ".architect/CENSUS.md"
  ".architect/AUDIT.md"
  ".architect/PLAN.md"
  ".worktrees/"
  ".fix/state-*.json"
  ".feature/state-*.json"
  ".feature/state.json"
  ".fix/state.json"
)

if $DRY_RUN; then
  echo "  Would add to .gitignore:"
  for entry in "${ENTRIES[@]}"; do
    echo "    $entry"
  done
else
  touch "$GITIGNORE"

  ADDED=0
  for entry in "${ENTRIES[@]}"; do
    if ! grep -qF "$entry" "$GITIGNORE" 2>/dev/null; then
      # Add a conductor section header if this is the first addition
      if [ "$ADDED" -eq 0 ]; then
        echo "" >> "$GITIGNORE"
        echo "# Conductor runtime state" >> "$GITIGNORE"
      fi
      echo "$entry" >> "$GITIGNORE"
      ADDED=$((ADDED + 1))
    fi
  done

  if [ "$ADDED" -gt 0 ]; then
    echo "  Added $ADDED entries to .gitignore"
  else
    echo "  .gitignore already up to date."
  fi
fi

# --- Done ---
echo ""
echo "=== Installation complete ==="
echo ""
echo "Installed conductors:"
echo "  .fix/launch.sh start 'bug description'     — TDD fix pipeline"
echo "  .feature/launch.sh start 'feature spec'     — feature pipeline"
echo "  .guardian/launch.sh                          — autonomous cleanup"
echo "  .architect/launch.sh                         — architecture audit"
echo ""
echo "Prerequisites:"
echo "  - agent-deck (npm i -g agent-deck)"
echo "  - claude CLI (Claude Code)"
echo ""
echo "Skills installed to: $SKILLS_DIR"
