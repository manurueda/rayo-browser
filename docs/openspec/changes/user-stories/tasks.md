# Tasks

## Module 1: New Assertion Types

- [ ] Add JsEvalAssertion, ElementStateAssertion, CookieAssertion structs to types.rs
- [ ] Add js_eval, element_state, no_console_errors, cookie_contains fields to Assertion struct
- [x] Add NetworkInterceptor to run_suite (Arc<Mutex<NetworkInterceptor>>)
- [x] Add network_mock step + mock response structs to TestStep
- [x] Implement check_network_called (replace stub)
- [ ] Implement check_js_eval (page.evaluate + comparison)
- [ ] Implement check_element_state (JS snippet for disabled/checked/visible/value)
- [ ] Implement check_no_console_errors (window.__rayoConsoleErrors)
- [ ] Implement check_cookie_contains (page.get_cookies + search)
- [x] Extract run_steps_on_page from run_suite for reuse by story runner
- [ ] Wire suite setup/teardown through story execution so network mocks apply to stories
- [ ] Unit tests for all assertion checkers
- [ ] YAML parse roundtrip tests for new assertion syntax

## Module 2: Story Types + Discovery

- [ ] Create story_types.rs (UserStory, StoryFlow, StoryAssertion)
- [ ] Create discover/stories.rs (story discovery algorithm)
- [ ] Add load_stories to loader.rs
- [ ] Add generate_story_files + write_story_files to generator.rs
- [ ] Integrate story discovery as Phase 3.5 in discover pipeline
- [ ] Update DiscoverResult with stories_generated field
- [ ] Unit tests for story discovery chaining/auth-gate logic
- [ ] YAML roundtrip tests for UserStory

## Module 3: Story Runner

- [ ] Create story_runner.rs
- [ ] Implement precondition resolution (topological sort + cycle detection)
- [ ] Implement run_story with shared browser session
- [ ] Implement precondition memoization (skip already-completed stories)
- [ ] Add StoryResult, StoryFlowResult, StoryAssertionResult types
- [ ] Unit tests for topological sort + cycle detection

## Module 4: CLI + Dashboard

- [ ] Add Stories subcommand to lib.rs CLI
- [ ] Update Scan command to include stories
- [ ] Add StoryResultPersisted to persistence.rs, extend ScanResult
- [ ] Add story-level narrative functions to narrative.rs
- [ ] Add /stories route + /api/stories to server.rs
- [ ] Create stories.html + story_detail.html askama templates
- [ ] Add Stories section to HTML scan report
- [ ] Update scan.rs to run stories after flows
