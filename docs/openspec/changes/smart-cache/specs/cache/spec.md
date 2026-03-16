# Smart Cache

## MODIFIED Requirements

### Requirement: Selector cache is consulted before page_map regeneration

The resolve_selector method SHALL check the selector cache before falling back to page_map regeneration. If a cached selector exists for the requested element ID, it MUST be returned directly without triggering a new page_map generation.

#### Scenario: Cached selector avoids page_map refresh
Given a page_map was generated and cached a selector for element id 5
When resolve_selector is called with id 5 again
Then the cached selector is returned without regenerating the page_map

### Requirement: Click invalidates selector cache but preserves page_map cache

Click actions SHALL invalidate the selector cache (incrementing the generation counter) but MUST NOT clear the page_map cache. Since most clicks do not structurally change the DOM, preserving the page_map avoids an unnecessary regeneration on the next observe call.

#### Scenario: Click keeps page_map cache for next observe
Given a page_map was just generated
When click is called on an element
Then the selector cache generation is incremented
But the page_map cache is preserved (not cleared to None)
