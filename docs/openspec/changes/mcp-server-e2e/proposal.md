# MCP Server End-to-End: Wire chromiumoxide + implement all 6 tools

## Problem
The MCP server scaffold exists but no tools are functional. The browser connection, page map extraction, batch execution, and profiling integration are all stubs.

## Solution
Wire chromiumoxide into rayo-core, implement all 6 MCP tools with real browser control, add integration tests, and produce a working `rayo-mcp` binary that Claude Code can use.

## Scope
1. rayo-core: Browser/Page wrapper around chromiumoxide with page_map extraction, batch execution
2. rayo-mcp: Wire rmcp server with all 6 tools functional
3. Integration tests for each tool
4. CLAUDE.md for the project
