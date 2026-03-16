# Event-Driven Waits

## MODIFIED Requirements

### Requirement: Wait resolves instantly when element already exists

The wait_for_selector implementation SHALL first check if the target element is already present in the DOM before installing any observer. If the element exists, the wait MUST resolve immediately without any polling delay.

#### Scenario: Element present before wait
Given a page with an existing button element
When wait_for_selector is called for that button
Then the wait resolves immediately without polling

### Requirement: Wait detects element via MutationObserver

When the target element is not yet present, wait_for_selector SHALL install a MutationObserver that resolves a Promise the instant the element appears in the DOM. This replaces the 50ms polling loop with event-driven detection, reducing wait latency from ~55-60ms to near-instant.

#### Scenario: Element appears after wait starts
Given a page without a target element
And the element will be inserted after 10ms
When wait_for_selector is called with a 5000ms timeout
Then the wait resolves within 25ms (not 60ms from polling)

### Requirement: Wait respects timeout

The wait_for_selector implementation SHALL enforce the configured timeout. If the element does not appear within the timeout period, the wait MUST return a timeout error and clean up the MutationObserver.

#### Scenario: Element never appears
Given a page without a target element
When wait_for_selector is called with a 100ms timeout
Then the wait returns a timeout error after ~100ms
