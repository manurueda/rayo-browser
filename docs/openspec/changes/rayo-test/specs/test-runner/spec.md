# Test Runner

## ADDED Requirements

### Requirement: YAML test definitions are parsed into executable test suites

The test runner SHALL parse `.rayo/tests/*.test.yaml` files into TestSuite structures containing name, viewport config, setup steps, and test cases with steps and assertions.

#### Scenario: Valid YAML test file is parsed
Given a file at .rayo/tests/login.test.yaml with valid test definition
When the parser loads the file
Then a TestSuite is returned with name, steps, and assertions
And each step has an action type and parameters

#### Scenario: Invalid YAML produces parse error
Given a file with invalid YAML syntax
When the parser attempts to load it
Then a descriptive parse error is returned with file path and line number

### Requirement: Steps are executed sequentially via rayo-core

The StepExecutor SHALL execute each test step by calling the corresponding rayo-core function (navigate, click, type, select, scroll, hover, press, wait, batch). Each step records timing, success/failure, and optional artifacts.

#### Scenario: Navigate step executes and records timing
Given a test step with action "navigate" and url "https://example.com"
When the step executes
Then rayo-core navigate is called with the URL
And the step result includes duration_ms and pass=true

#### Scenario: Failed step records error
Given a test step clicking a non-existent selector
When the step executes
Then the step result includes pass=false and an error message

### Requirement: Assertions verify page state after step execution

The assertion engine SHALL support: page_map_contains (element exists by selector/text/role), text_contains (page text includes expected string), screenshot_matches (visual comparison against baseline), and network_called (API request was captured).

#### Scenario: page_map_contains passes when element exists
Given a page with a button containing text "Submit"
When page_map_contains assertion checks for text "Submit"
Then the assertion passes

#### Scenario: page_map_contains fails when element missing
Given a page without a "Checkout" button
When page_map_contains assertion checks for text "Checkout"
Then the assertion fails with message indicating element not found

#### Scenario: screenshot_matches auto-creates baseline on first run
Given no baseline exists for name "dashboard"
When screenshot_matches assertion runs
Then a new baseline is saved
And the assertion passes with new_baseline=true flag

### Requirement: Test results are collected into structured reports

The ResultCollector SHALL aggregate per-step results into a TestSuiteResult with overall pass/fail, total duration, and per-test/per-step breakdowns. Reports are generated in JSON and HTML formats.

#### Scenario: Successful suite produces JSON report
Given a test suite where all steps and assertions pass
When the suite completes
Then a JSON report is generated with pass=true and per-step timing
And the report includes the suite name and total duration

#### Scenario: Failed assertion produces report with failure details
Given a test suite where one assertion fails
When the suite completes
Then the report includes pass=false
And the failing step includes the assertion error message and page map at time of failure

### Requirement: Web server exposes test results via REST API and WebSocket

An axum server SHALL serve: GET /api/suites (list all), GET /api/results/:id (suite result), POST /api/run (trigger suite execution), WS /ws/live (real-time step-by-step updates during execution).

#### Scenario: GET /api/suites lists available test files
Given 3 test YAML files in .rayo/tests/
When GET /api/suites is called
Then a JSON array of 3 suite summaries is returned

#### Scenario: WebSocket streams live step execution
Given a test suite is running
When a client connects to /ws/live
Then each step completion is sent as a WebSocket message
And the message includes step name, pass/fail, and timing
