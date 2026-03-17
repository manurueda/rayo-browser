# Tasks

- [ ] Add `state: Vec<String>` field to `InteractiveElement` in `page_map.rs` with `skip_serializing_if = "Vec::is_empty"`
- [ ] Add `total_interactive: Option<usize>` and `truncated: Option<bool>` fields to `PageMap` in `page_map.rs` with `skip_serializing_if = "Option::is_none"`
- [ ] Update `EXTRACT_PAGE_MAP_JS` to detect element states (disabled, readonly, required, checked, hidden) and report truncation metadata
- [ ] Update scoped page_map JS in `browser.rs` with the same state detection and truncation logic
- [ ] Update unit test in `page_map.rs` to include the new `state` field in `InteractiveElement` construction
- [ ] Verify: `cargo build --workspace && cargo test --workspace && cargo clippy --workspace`
