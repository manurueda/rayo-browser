# Web UI

## ADDED Requirements

### Requirement: Dashboard shows test health overview

The dashboard page SHALL display: overall pass rate, last run summary (pass/fail/total), trend chart of recent runs, and average test execution speed.

#### Scenario: Dashboard with recent test runs
Given 10 test suite runs have completed
When the dashboard loads
Then it displays the overall pass rate as a percentage
And a trend chart showing pass/fail over the last 10 runs
And the average execution time

### Requirement: Suite list shows all test suites with status

The suite list page SHALL display all test suites from .rayo/tests/ with their last run status (pass/fail/never run), duration, and test count.

#### Scenario: Suite list with mixed results
Given 3 test suites where 2 passed and 1 failed
When the suite list loads
Then all 3 suites are listed with their status
And the failed suite is visually distinct (red indicator)
And each suite shows its last run duration

### Requirement: Test detail view shows step-by-step execution

The test detail page SHALL display each step with: name, action type, duration, pass/fail, page map snapshot, screenshot (if captured), assertion results, and diff overlay (if visual assertion failed).

#### Scenario: Viewing a failed test with visual diff
Given a test run where a screenshot assertion failed
When the test detail page loads
Then each step is listed in execution order
And the failing step shows the baseline image, current image, and diff overlay
And the diff overlay highlights changed regions in red

### Requirement: Live runner streams execution in real-time

The live runner page SHALL connect via WebSocket to the rayo-test server and display each step as it executes with real-time status updates, timing, and progress indication.

#### Scenario: Live execution of a 5-step test
Given a test suite with 5 steps is triggered
When the live runner page is open
Then each step appears as it starts executing
And completed steps show pass/fail and duration
And a progress bar indicates overall completion
