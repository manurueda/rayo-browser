# Observation Layer

## MODIFIED Requirements

### Requirement: text_content returns all matching elements

When a CSS selector is provided, text_content() SHALL use querySelectorAll to return text from ALL matching elements joined by newlines, not just the first match. A max_elements parameter (default 50) SHALL cap the number of elements, appending a truncation warning when exceeded.

#### Scenario: Multiple elements match selector
Given a page with 5 elements matching ".card"
When text_content is called with selector ".card" and max_elements 50
Then the result contains text from all 5 elements joined by newlines

#### Scenario: No elements match selector
Given a page with no elements matching ".nonexistent"
When text_content is called with selector ".nonexistent"
Then the result is an empty string (not a crash)

#### Scenario: Elements exceed max_elements cap
Given a page with 100 elements matching "p"
When text_content is called with selector "p" and max_elements 10
Then the result contains text from 10 elements
And the result ends with a truncation warning

### Requirement: page_map supports selector scoping

When a CSS selector is provided to page_map(), interactive elements, headings, and text_summary SHALL be scoped to the matched DOM subtree. If the selector does not match any element, an empty page map is returned.

#### Scenario: Scoped page_map filters to subtree
Given a page with interactive elements inside and outside ".sidebar"
When page_map is called with selector ".sidebar"
Then only interactive elements within the sidebar subtree are returned

#### Scenario: Non-matching selector returns empty page_map
Given a page with no element matching ".nonexistent"
When page_map is called with selector ".nonexistent"
Then an empty interactive array is returned (not a crash)

## ADDED Requirements

### Requirement: Batch abort_on_failure stops execution

When abort_on_failure is true, execute_batch() SHALL stop on the first failed action and mark all remaining actions as skipped. The default is false (existing behavior preserved).

#### Scenario: Abort on failure skips remaining actions
Given a batch of 5 actions with abort_on_failure true
When action 2 fails
Then actions 3, 4, 5 are marked as skipped with error "Skipped (abort_on_failure)"

### Requirement: Network capture records real CDP requests

rayo_network capture mode SHALL wire CDP Fetch.requestPaused events to record real network requests into the NetworkInterceptor. Block rules SHALL use Fetch.failRequest and mock rules SHALL use Fetch.fulfillRequest.

#### Scenario: Captured requests are non-empty after navigation
Given network capture is started
When a page navigation occurs
Then rayo_network requests returns at least 1 captured request
