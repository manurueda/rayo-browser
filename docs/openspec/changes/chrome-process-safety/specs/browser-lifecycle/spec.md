# Browser Lifecycle

## ADDED Requirements

### Requirement: RayoBrowser provides async graceful shutdown

RayoBrowser SHALL expose a `close()` async method that gracefully shuts down Chrome by sending the CDP close command, waiting for the process to exit with a 5-second timeout, and force-killing if the timeout expires.

#### Scenario: Graceful close shuts down Chrome cleanly
Given a RayoBrowser instance with a running Chrome process
When close() is called
Then a CDP close command is sent to Chrome
And the method waits up to 5 seconds for Chrome to exit
And the handler task is aborted and awaited

#### Scenario: Force-kill on timeout
Given a RayoBrowser instance where Chrome does not respond to the close command
When close() is called and 5 seconds elapse without Chrome exiting
Then Chrome is force-killed via the kill method
And the handler task is aborted

## MODIFIED Requirements

### Requirement: MCP server performs graceful browser shutdown on exit

The MCP server SHALL call `browser.close()` after the MCP session ends, before the process exits. This prevents zombie Chrome processes when the MCP transport disconnects.

#### Scenario: MCP session ends cleanly
Given a running rayo-mcp server with an active Chrome browser
When the MCP session ends (transport disconnects)
Then browser.close() is called to gracefully shut down Chrome
And the server process exits without leaving orphan Chrome processes

### Requirement: Drop impl acts as safety net

The RayoBrowser Drop impl SHALL abort the handler task as a safety net for cases where `close()` was not called. chromiumoxide's own Drop handles killing the Chrome child process.

#### Scenario: Browser dropped without explicit close
Given a RayoBrowser instance that was not explicitly closed
When the RayoBrowser is dropped
Then the handler task is aborted
And chromiumoxide's kill_on_drop handles the Chrome process cleanup
