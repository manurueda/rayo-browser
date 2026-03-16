# Remove Dead Types: Clean up unused types across rayo-core and rayo-rules

## Why
Multiple types are defined but never used anywhere in the codebase:
- 5 option types in actions.rs (ActionResult, NavigateOptions, ClickOptions, TypeOptions, ScreenshotOptions)
- 2 wait types in wait.rs (WaitStrategy, WaitConfig)
- 3 error variants (NavigationFailed, BatchActionFailed, PageNotAvailable)
- 1 dead module in rayo-rules (suggestions.rs)

These add ~150 lines of dead weight that confuse API surface understanding.

## Solution
Delete unused types, variants, and the dead module. Also fix the misleading `remote_object_id` parameter name in SelectorCache.

## Scope

### rayo-core
- `actions.rs` — remove 5 unused option/result types
- `wait.rs` — remove WaitStrategy and WaitConfig (keep module if tests reference it, otherwise remove)
- `error.rs` — remove NavigationFailed, BatchActionFailed, PageNotAvailable variants
- `selector_cache.rs` — rename `remote_object_id` param to `resolved_selector`
- `lib.rs` — clean up any broken exports

### rayo-rules
- Delete `suggestions.rs` and remove `pub mod suggestions` from lib.rs

## Not in scope
- Adding new types to replace removed ones
- Re-exporting PageMap/BatchAction (separate concern)
