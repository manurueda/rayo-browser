# Event-Driven Waits: MutationObserver instead of polling

## Why
`wait_for_selector()` polls every 50ms with a CDP round-trip each iteration (~55-60ms per attempt). Average detection latency is half the poll interval plus one round-trip, wasting 25-55ms per wait.

## Solution
Replace polling with a MutationObserver-based Promise that resolves the instant the target element appears in the DOM.

## Scope

### rayo-core
- `browser.rs` — `wait_for_selector()`: replace polling loop with single CDP evaluate of MutationObserver Promise
- `wait.rs` — wire existing WaitStrategy/WaitConfig into actual wait code

## Not in scope
- Smart cache invalidation (separate change)
- CDP DOM event subscriptions for cache (separate change)
