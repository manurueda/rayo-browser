# Profiler Safety

## MODIFIED Requirements

### Requirement: Profiler operations never panic on poisoned mutex

All public profiler methods SHALL handle poisoned mutex gracefully instead of calling .unwrap(). A poisoned mutex MUST result in a no-op or default return value, not a panic that crashes the MCP server.

#### Scenario: Profiler continues after thread panic
Given a thread panicked while holding the profiler collector lock
When another thread calls start_span()
Then the profiler does not panic
And the span is silently dropped

#### Scenario: Spans query returns empty on poisoned mutex
Given the profiler collector mutex is poisoned
When spans() is called
Then an empty vec is returned instead of a panic
