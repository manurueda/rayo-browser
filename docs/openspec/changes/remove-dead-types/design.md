# Design: Remove Dead Types

## actions.rs
Remove types: ActionResult, NavigateOptions, ClickOptions, TypeOptions, ScreenshotOptions.
Keep only: ActionTarget (used by BatchAction).

## wait.rs
Remove: WaitStrategy, WaitConfig. Keep the module file if it has tests; remove entirely if empty after cleanup.

## error.rs
Remove variants: NavigationFailed, BatchActionFailed, PageNotAvailable.
Keep: ElementNotFound, Timeout, Cdp, CookieError, Chromiumoxide, Other.

## selector_cache.rs
Rename `remote_object_id` parameter in `put()` and field in `CachedElement` to `resolved_selector`.

## rayo-rules/suggestions.rs
Delete file and remove `pub mod suggestions` from lib.rs.
