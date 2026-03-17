# Test Coverage

## ADDED Requirements

### Requirement: Page map truncation is testable on large pages

A many_elements.html fixture with 100+ interactive elements SHALL exist so tests can verify that page_map correctly sets truncated=true, total_interactive > 50, and caps interactive.len() at 50.

#### Scenario: Page map truncates at MAX_ELEMENTS
Given a page with 110 interactive elements (60 inputs, 30 buttons, 20 links)
When page_map is called
Then interactive.len() == 50 and truncated == true and total_interactive > 50

### Requirement: Element state detection is tested

Integration tests SHALL verify that disabled, readonly, and required element states appear in the page_map state arrays for form elements with those attributes.

#### Scenario: Disabled element has disabled state
Given a form page with a disabled input
When page_map is called
Then the disabled input has "disabled" in its state array

#### Scenario: Readonly element has readonly state
Given a form page with a readonly input
When page_map is called
Then the readonly input has "readonly" in its state array

#### Scenario: Required element has required state
Given a form page with a required input
When page_map is called
Then the required input has "required" in its state array

### Requirement: Click on non-existent element returns ElementNotFound

Clicking a CSS selector that matches no element SHALL return RayoError::ElementNotFound with the selector in the error message.

#### Scenario: Click non-existent selector
Given a page with no element matching "#does-not-exist"
When click is called with selector "#does-not-exist"
Then RayoError::ElementNotFound is returned containing the selector string

### Requirement: Empty page produces valid empty page map

Navigating to about:blank and calling page_map SHALL return a PageMap with an empty interactive array and truncated not set.

#### Scenario: about:blank has no interactive elements
Given the browser is on about:blank
When page_map is called
Then interactive is empty and truncated is None

### Requirement: Batch mixed results are reported correctly

When a batch contains both valid and invalid actions with abort_on_failure=false, the BatchResult SHALL report correct succeeded and failed counts, and all actions SHALL execute regardless of earlier failures.

#### Scenario: Batch with one failing action
Given a batch of [screenshot, click-nonexistent, screenshot] with abort_on_failure=false
When the batch is executed
Then succeeded == 2 and failed == 1 and all three results are present

### Requirement: Wait for visibility times out on hidden elements

wait_for_selector with visible=true SHALL time out if the element exists in the DOM but has display:none. With visible=false it SHALL succeed.

#### Scenario: Hidden element times out with visible=true
Given a page with a display:none element
When wait_for_selector is called with visible=true and timeout 500ms
Then a Timeout error is returned

#### Scenario: Hidden element found with visible=false
Given a page with a display:none element
When wait_for_selector is called with visible=false
Then it succeeds (element exists in DOM)
