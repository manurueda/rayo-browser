# Tasks

- [ ] Add invalidate_after_mutation() method to RayoPage
- [ ] Call it from click(), type_text(), select_option()
- [ ] Update execute_batch() to also clear page_map_cache at end
- [ ] Refactor resolve_selector() to use single lock acquisition
- [ ] Replace .ok() in type_text clear with tracing::warn on failure
- [ ] Verify build, lint, and tests pass
