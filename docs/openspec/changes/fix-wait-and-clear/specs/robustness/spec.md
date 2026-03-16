# Wait and Clear Robustness

## MODIFIED Requirements

### Requirement: MutationObserver handles null document.body

The wait_for_selector implementation SHALL fall back to `document.documentElement` when `document.body` is null, preventing TypeError crashes during early page load.

#### Scenario: Wait succeeds during early page load
Given document.body has not yet been parsed
When wait_for_selector is called
Then the MutationObserver attaches to document.documentElement instead
And the wait does not throw a TypeError

### Requirement: Text clearing uses non-deprecated APIs

The type_text clear logic SHALL use direct value assignment with proper event dispatch instead of the deprecated document.execCommand('delete') API.

#### Scenario: Clear replaces existing text
Given an input element with value "old text"
When type_text is called with clear=true and value "new text"
Then the input value becomes "new text"
And input and change events are dispatched
