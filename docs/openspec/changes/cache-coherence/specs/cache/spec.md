# Cache Coherence

## MODIFIED Requirements

### Requirement: All DOM mutation methods invalidate page_map cache

Every method that mutates DOM state (click, type_text, select_option) SHALL invalidate the page_map_cache so that subsequent page_map() calls return fresh data reflecting the mutation.

#### Scenario: type_text invalidates page_map cache
Given a page_map was generated showing input value "old"
When type_text is called with value "new" and clear=true
Then the page_map_cache is invalidated
And the next page_map() call returns the updated input value

#### Scenario: select_option invalidates page_map cache
Given a page_map was generated showing select value "option1"
When select_option is called with value "option2"
Then the page_map_cache is invalidated

### Requirement: Cache invalidation uses a centralized method

All DOM mutation methods SHALL use a single `invalidate_after_mutation()` method instead of inline cache operations. This ensures consistent invalidation behavior and makes the policy explicit.

#### Scenario: Centralized invalidation is called from all mutation methods
Given click(), type_text(), and select_option() all mutate the DOM
When any of these methods completes
Then invalidate_after_mutation() is called
And both selector_cache and page_map_cache are invalidated
