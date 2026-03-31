# Architecture Review Checklist

1. Is there a clear composition root?
2. Which modules do I/O?
3. Which modules hold mutable state?
4. Which modules are pure policy?
5. Which values are durable vs per-turn?
6. Are invalidation paths explicit?
7. Can later steps observe earlier writes when they should?
8. Are fallbacks runtime-driven, not UI-driven?
9. Are timing or profiling phases semantically clean?
10. Is any helper module mixing unrelated domains?
