# Cookie Management

## ADDED Requirements

### Requirement: rayo_cookie MCP tool provides set, get, and clear actions for browser cookies

The MCP server SHALL expose a `rayo_cookie` tool that allows AI agents to manage browser cookies. The tool MUST support setting cookies with full attribute control (domain, path, secure, httpOnly, sameSite, expires), reading cookies with optional domain filtering, and clearing cookies either globally or by domain.

#### Scenario: Set cookies for authenticated access
- Given the browser has navigated to an HTTP page
- When the agent calls rayo_cookie with action "set" and a cookies array containing name, value, and domain
- Then the cookies are injected into the browser session via CDP
- And subsequent navigations include those cookies in requests

#### Scenario: Get cookies with optional domain filter
- Given cookies have been set on the page
- When the agent calls rayo_cookie with action "get"
- Then all cookies for the current page are returned as JSON
- And if a domain filter is provided, only matching cookies are returned

#### Scenario: Clear all cookies
- Given cookies exist in the browser session
- When the agent calls rayo_cookie with action "clear" without a domain filter
- Then all cookies are removed via a single CDP ClearBrowserCookies call

#### Scenario: Clear cookies by domain
- Given cookies exist for multiple domains
- When the agent calls rayo_cookie with action "clear" and a domain filter
- Then only cookies matching that domain are deleted
- And cookies for other domains remain untouched

### Requirement: rayo-core exposes cookie operations through owned types without leaking CDP types

Cookie operations in rayo-core MUST use owned types (`SetCookie`, `CookieInfo`, `SameSite`) defined in `rayo-core::cookie`. Chromiumoxide CDP types (`CookieParam`, `Cookie`, `TimeSinceEpoch`, `CookieSameSite`) SHALL be converted internally within `browser.rs` and MUST NOT appear in rayo-core's public API.

#### Scenario: Boundary isolation from chromiumoxide
- Given rayo-mcp imports cookie types from rayo-core
- Then those types are SetCookie, CookieInfo, and SameSite defined in rayo-core
- And no chromiumoxide types appear in rayo-core's public API for cookies
