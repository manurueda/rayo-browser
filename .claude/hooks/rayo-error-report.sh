#!/bin/bash
# Claude Code PostToolUseFailure hook for rayo MCP tools.
# Logs errors to ~/.rayo/error-log.jsonl and injects context for Claude.

set -euo pipefail

INPUT=$(cat)

TOOL_NAME=$(echo "$INPUT" | jq -r '.tool_name // "unknown"')
ERROR=$(echo "$INPUT" | jq -r '.error // "unknown error"')
TOOL_INPUT=$(echo "$INPUT" | jq -c '.tool_input // {}')

# Persist to error log
LOG_DIR="${HOME}/.rayo"
mkdir -p "$LOG_DIR"
LOG_FILE="${LOG_DIR}/error-log.jsonl"

jq -n \
  --arg ts "$(date -u +%Y-%m-%dT%H:%M:%SZ)" \
  --arg tool "$TOOL_NAME" \
  --arg error "$ERROR" \
  --argjson input "$TOOL_INPUT" \
  '{timestamp: $ts, tool: $tool, error: $error, input: $input}' >> "$LOG_FILE"

# Count recent errors for this tool (last 10 minutes)
RECENT_COUNT=$(tail -50 "$LOG_FILE" 2>/dev/null | jq -r --arg tool "$TOOL_NAME" \
  'select(.tool == $tool) | .timestamp' | wc -l | tr -d ' ')

# Inject context back to Claude via stdout
if [ "$RECENT_COUNT" -ge 3 ]; then
  echo "⚡ rayo tool '$TOOL_NAME' has failed ${RECENT_COUNT} times recently. Consider:"
  echo "1. Call rayo_report to get structured error data"
  echo "2. File an issue: gh issue create --repo manurueda/rayo-browser --title 'Bug: $TOOL_NAME failure' --body '<paste rayo_report output>'"
else
  echo "⚡ rayo error logged to ~/.rayo/error-log.jsonl"
fi
