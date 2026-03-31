# rayo-ui user stories

## ADDED Requirements

### Requirement: User story discovery and execution

The system MUST discover user stories by chaining detected flows based on auth gates and URL relationships, MUST generate `*.story.yaml` files, and SHALL execute stories with shared browser sessions, precondition resolution, and suite setup/teardown so deterministic network mocks can be applied.

#### Scenario: Discover stories from flows with auth gates

Given an app with a login page and auth-gated dashboard
When `rayo-ui discover` runs
Then it generates `*.story.yaml` files chaining login → dashboard flows
And the auth story has importance "critical"

#### Scenario: Run stories with preconditions

Given a story that requires "login Login Flow"
When the story runner executes it
Then the login flow runs first on a shared browser session
And subsequent flows inherit the login cookies

#### Scenario: Suite setup can mock deterministic network responses

Given a test suite setup step with `network_mock` for `/api/stream`
When the suite executes
Then matching requests are fulfilled with the configured mock response
And later assertions can verify the request was captured with `network_called`

#### Scenario: Non-visual assertions verify app state

Given a test step with `no_console_errors: true` assertion
When the step executes on a page with zero console errors
Then the assertion passes

Given a test step with `js_eval` assertion `expression: "document.title"`
When the step executes
Then the page title is evaluated and compared against the expected value

Given a test step with `element_state` assertion for a disabled button
When the element is found and is disabled
Then the assertion passes

Given a test step with `cookie_contains` assertion for "session" cookie
When the cookie exists
Then the assertion passes

### Requirement: Story YAML format

Stories MUST be defined in `*.story.yaml` files with name, description, persona, importance, requires (preconditions), and flows (referencing TestSuite names with human-readable then-assertions).

#### Scenario: Parse minimal story YAML

Given a YAML file with name and one flow
When parsed as UserStory
Then all optional fields default to empty/None

#### Scenario: Story results displayed for non-developers

Given story execution results
When rendered in the dashboard or scan report
Then each story shows pass/fail with plain-English descriptions
And no CSS selectors or code are visible
