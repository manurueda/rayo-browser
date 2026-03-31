#!/bin/bash
# Mark a task as completed in WORK_QUEUE.md
#
# Usage:
#   .guardian/mark-complete.sh "task description snippet"
#   .guardian/mark-complete.sh --skip "task description snippet" "reason"
#
# Finds the first `- [ ]` line containing the snippet, marks it [x] or [S],
# and updates the Status counts. Atomic file operation.

set -euo pipefail

QUEUE="$(git rev-parse --show-toplevel)/.guardian/WORK_QUEUE.md"
TIMESTAMP=$(date -u +"%Y-%m-%dT%H:%M:%SZ")

MODE="complete"
REASON=""

if [ "${1:-}" = "--skip" ]; then
  MODE="skip"
  shift
  SNIPPET="${1:?Usage: mark-complete.sh --skip \"snippet\" \"reason\"}"
  REASON="${2:?Usage: mark-complete.sh --skip \"snippet\" \"reason\"}"
else
  SNIPPET="${1:?Usage: mark-complete.sh \"task description snippet\"}"
fi

if [ ! -f "$QUEUE" ]; then
  echo "ERROR: $QUEUE not found"
  exit 1
fi

python3 -c "
import re, sys

queue_path = '$QUEUE'
snippet = '''$SNIPPET'''
mode = '$MODE'
reason = '''$REASON'''
timestamp = '$TIMESTAMP'

with open(queue_path, 'r') as f:
    lines = f.readlines()

found = False
for i, line in enumerate(lines):
    # Match pending tasks containing the snippet
    if line.strip().startswith('- [ ]') and snippet in line:
        if mode == 'complete':
            lines[i] = line.rstrip() + f' (completed {timestamp})\n'
            lines[i] = lines[i].replace('- [ ]', '- [x]', 1)
        else:
            lines[i] = line.rstrip() + f' (skipped: {reason})\n'
            lines[i] = lines[i].replace('- [ ]', '- [S]', 1)
        found = True
        break

if not found:
    print(f'WARN: no pending task matching \"{snippet}\" found in queue')
    sys.exit(0)

# Update status counts
completed = sum(1 for l in lines if l.strip().startswith('- [x]'))
pending = sum(1 for l in lines if l.strip().startswith('- [ ]'))
skipped = sum(1 for l in lines if l.strip().startswith('- [S]'))

for i, line in enumerate(lines):
    if line.startswith('- Completed:'):
        lines[i] = f'- Completed: {completed}\n'
    elif line.startswith('- Pending:'):
        lines[i] = f'- Pending: {pending}\n'
    elif line.startswith('- Skipped:'):
        lines[i] = f'- Skipped: {skipped}\n'

with open(queue_path, 'w') as f:
    f.writelines(lines)

action = 'completed' if mode == 'complete' else f'skipped: {reason}'
print(f'OK: marked [{\"x\" if mode == \"complete\" else \"S\"}] — {action} ({completed} done, {pending} pending, {skipped} skipped)')
"
