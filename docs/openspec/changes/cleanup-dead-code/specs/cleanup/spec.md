# Dead Code Cleanup

## MODIFIED Requirements

### Requirement: MCP tool modules contain only active code

The rayo-mcp tools directory SHALL contain only files with active handler logic. Unused type definition files and duplicate handler implementations MUST be removed to prevent maintenance divergence.

#### Scenario: No duplicate cookie handler
Given the cookie handler logic exists in tools/mod.rs
When tools/cookie.rs is deleted
Then all cookie operations continue to work via mod.rs handle_cookie()
And no compilation errors occur

#### Scenario: Tool descriptions match implementation
Given screenshots now use JPEG format
When the rayo_observe tool description is read by an AI agent
Then it correctly states "base64 JPEG" not "base64 PNG"
