# Page Map Role Text

## ADDED Requirements

### Requirement: Page map includes text for ARIA role equivalents

When an interactive element is discovered via `role="button"`, `role="link"`, or `role="tab"`, the page map SHALL populate `item.text` from the element's visible text content using the same text-length limits applied to native buttons and links.

#### Scenario: Full page map includes role button text

Given a visible element with `role="button"` and text `Create project`
When the full page map is generated
Then the matching interactive element includes `text: "Create project"`

#### Scenario: Full page map includes role link text

Given a visible element with `role="link"` and text `Open docs`
When the full page map is generated
Then the matching interactive element includes `text: "Open docs"`

#### Scenario: Scoped page map includes role tab text

Given a subtree contains a visible element with `role="tab"` and text `Settings`
When a scoped page map is generated for that subtree
Then the matching interactive element includes `text: "Settings"`
