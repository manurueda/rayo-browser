# Content-Aware Waits

## Why

The current runner waits only support `selector` and `network_idle`. That works for element presence, but it breaks down for async UIs where the element exists before its content does, such as streaming chat, status banners, and progressive onboarding panes.

## Solution

Extend `WaitAction` with content-aware conditions so tests can wait on page text or element text without resorting to sleeps or fragile assertion timing:

- `text`: wait until the page contains the target text
- `element_text`: wait until a specific element contains the target text

## Scope

- `types.rs` and `runner.rs` in `rayo-ui`
- YAML parsing and roundtrip coverage for the new wait fields
- Tests that verify timeout behavior and content matching

## Not in scope

- Natural-language action resolution or `ai_check`
- Network mocking or request assertions
- Changes to browser-level selector waiting internals
