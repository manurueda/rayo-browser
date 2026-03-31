# rayo-ui content-aware waits

## ADDED Requirements

### Requirement: Wait can resolve on page text content

The runner SHALL support a `wait.text` condition that resolves when the page contains the requested text. Matching MUST be case-insensitive and use contains semantics rather than exact equality.

#### Scenario: Page text appears after action

Given a step that triggers an async message to appear
And the `wait` condition specifies `text: "is building"`
When the text appears within the timeout
Then the wait resolves successfully

### Requirement: Wait can resolve on element text content

The runner SHALL support a `wait.element_text` condition with `selector` and `contains` fields. The wait MUST resolve when the selected element's text content contains the requested text.

#### Scenario: Element text appears after rerender

Given a page with an empty status container
And the `wait` condition specifies `element_text.selector: ".status"` and `contains: "ready"`
When the container text becomes "ready"
Then the wait resolves successfully

### Requirement: Wait timeouts report the last observed text

If a content-aware wait times out, the runner SHALL return a failure message that includes the timeout duration and a truncated snippet of the last observed text to help diagnose why the wait did not resolve.

#### Scenario: Text never appears

Given a page that never shows the requested text
When a `wait.text` condition times out
Then the failure message includes the missing text and a snippet of the observed page text
