# SRP, DI, DRY

## SRP

Each module should answer:

- what it owns
- what changes make it change
- what it explicitly does not own

## DI

Inject dependencies when they do I/O, hold state, or need swapping in tests.

## DRY

Share code only when the invariant is real and the abstraction name is obvious.

## Cache Rule

Every cache should make clear:

- what is cached
- what makes it stale
- who invalidates it
- whether stale data can leak across turns or sessions
