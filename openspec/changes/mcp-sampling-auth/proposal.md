# MCP Sampling Auth: LLM-powered login page detection

## Problem

Heuristic auth detection (auth-detection-v2) covers ~90% of cases. The remaining 10% are pages where:
- Login is in a non-English language
- Custom branded OAuth with no standard text patterns
- JavaScript-rendered login forms not captured in page_map
- Novel auth patterns not covered by keyword lists

Rayo's philosophy: rock solid, then ultrafast.

## Solution

Use MCP sampling to ask the calling AI agent (which already has vision) to classify a screenshot as a login page. Zero new dependencies. Zero cost (uses the agent's existing model quota).

### Architecture

```
goto_with_auto_auth
  ├── heuristics: confidence >= 0.5 → auth (skip LLM)
  ├── heuristics: confidence < 0.2  → not auth (skip LLM)
  └── heuristics: 0.2 - 0.5        → UNCERTAIN
       ├── take screenshot
       ├── MCP sampling/createMessage → calling agent
       │   "Is this a login page? [screenshot]"
       │   ← "yes" / "no"
       └── fall back to heuristics if sampling unavailable/times out
```

### Dependency boundary

rayo-core cannot depend on rmcp (circular). Bridge via callback type alias:

```rust
// rayo-core/src/auth.rs
pub type LlmAuthChecker = Box<
    dyn Fn(String) -> Pin<Box<dyn Future<Output = Option<bool>> + Send>>
        + Send + Sync
>;
```

rayo-mcp constructs a closure capturing the peer handle:
```rust
// rayo-mcp/src/server.rs
let checker: LlmAuthChecker = Box::new(move |b64| {
    let peer = peer.clone();
    Box::pin(async move {
        peer.create_message(CreateMessageRequestParam { ... }).await
            .ok().map(|r| r.message.content_text().contains("yes"))
    })
});
```

### Sampling request
- System prompt: None (minimal tokens)
- User message: screenshot (JPEG base64) + "Is this a login, sign-in, or authentication page? Answer only: yes or no"
- max_tokens: 5
- model_preferences: cost=1.0, speed=1.0, intelligence=0.0 (prefer cheap/fast)
- Timeout: 5 seconds

### Graceful degradation
- Client doesn't support sampling → `Option<LlmAuthChecker>` is None → heuristics only
- Sampling times out (5s) → returns None → heuristics with 0.35 threshold
- Screenshot fails → returns None → heuristics only
- Sampling returns garbage → `contains("yes")` → defaults to false

## Scope

### rayo-core
- `auth.rs` — `LlmAuthChecker` type alias (already defined in auth-detection-v2, unused)
- `browser.rs` — wire `llm_checker` parameter into the uncertain confidence branch

### rayo-mcp
- `server.rs` — store `Option<LlmAuthChecker>` on `RayoServer`, construct from peer handle after session init, check client `capabilities.sampling`
- `tools/mod.rs` — pass `llm_checker.as_ref()` from server to `handle_navigate` to `page.goto_with_auto_auth`

## Boundary decisions
- LlmAuthChecker stored on RayoServer, constructed once at session start
- rmcp already has full sampling support (CreateMessageRequest, Peer::create_message)
- No reqwest dependency, no API key, no external LLM call
- Only fires in the uncertain confidence zone (0.2-0.5) — zero overhead on happy path and clear auth/non-auth cases

## Not in scope
- Using sampling for other purposes (element selection, page understanding)
- Configurable sampling prompts
- Caching LLM auth decisions across navigations
