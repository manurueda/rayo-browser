# Page Map Metadata

## ADDED Requirements

### Requirement: Interactive elements report their state

Each interactive element in the page map SHALL include a `state` array containing applicable states from the set: `disabled`, `readonly`, `required`, `checked`, `hidden`. The `state` field is omitted when empty to preserve token efficiency.

#### Scenario: Disabled button includes disabled state
Given a page contains a button with the `disabled` attribute
When page_map is generated
Then the button element's `state` array includes "disabled"

#### Scenario: Required input includes required state
Given a page contains an input with the `required` attribute
When page_map is generated
Then the input element's `state` array includes "required"

#### Scenario: Normal element omits state field
Given a page contains an interactive element with no special states
When page_map is generated
Then the element does not include a `state` field in the JSON output

### Requirement: Page map reports truncation metadata when elements are capped

The page map SHALL include `total_interactive` and `truncated` fields when the number of interactive elements exceeds the MAX_ELEMENTS cap (50). These fields are omitted when no truncation occurred.

#### Scenario: Truncated page includes total count
Given a page contains 80 interactive elements
When page_map is generated with a MAX_ELEMENTS cap of 50
Then the response includes `total_interactive: 80` and `truncated: true`
And the `interactive` array contains exactly 50 elements

#### Scenario: Non-truncated page omits truncation fields
Given a page contains 30 interactive elements
When page_map is generated
Then the response does not include `total_interactive` or `truncated` fields

### Requirement: Scoped page map includes state and truncation metadata

When page_map is generated for a CSS-selected subtree (scoped mode), the same element state detection and truncation metadata SHALL be applied as in the full page map.

#### Scenario: Scoped page map detects element states
Given a scoped page_map is requested for a form containing a disabled input
When the scoped page_map is generated
Then the disabled input's `state` array includes "disabled"
