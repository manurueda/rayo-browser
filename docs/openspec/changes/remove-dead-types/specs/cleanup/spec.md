# Dead Type Removal

## MODIFIED Requirements

### Requirement: No unused type definitions in public API

All types defined in rayo-core and rayo-rules SHALL be used by at least one caller. Unused scaffolding types MUST be removed to keep the API surface accurate and discoverable.

#### Scenario: Removed types do not break compilation
Given ActionResult, NavigateOptions, ClickOptions, TypeOptions, ScreenshotOptions are removed from actions.rs
And WaitStrategy, WaitConfig are removed from wait.rs
And NavigationFailed, BatchActionFailed, PageNotAvailable are removed from error.rs
When the workspace is compiled
Then no compilation errors occur
And all existing tests pass
