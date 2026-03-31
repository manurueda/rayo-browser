# Test Style Guide

Write tests like Kent Beck and Martin Fowler would: each test tells a story about one behaviour, the suite reads like a specification, and nothing is tested twice.

## Core Principles

1. **Test behaviour, not implementation.** Assert what the function *does*, not *how* it does it. If you refactor the internals, the tests should still pass.
2. **One behaviour per test.** Each `it()` block proves one thing. The test name is the documentation.
3. **No test should need a comment to explain itself.** If the name + assertion aren't enough, the test is too complex.
4. **Parameterize, don't enumerate.** Input variations belong in `it.each()`, not copied `it()` blocks.
5. **Minimise mocking.** Test real code paths whenever possible. Mocks are a last resort, not the default.

## File Rules

- **Maximum 300 lines per test file.** If you're over, split by behaviour group.
- **One `describe` per function or behaviour group.**
- **No `BUG:` prefix on test names.** If a test documents broken behaviour, use `test.skip()` with a `// TODO:` comment — never a passing test that asserts wrong behaviour.

## Input Validation: Use `it.each()`

Bad (50 lines for one concept):
```typescript
it('rejects null input', () => { ... });
it('rejects undefined input', () => { ... });
it('rejects empty string input', () => { ... });
it('rejects whitespace-only input', () => { ... });
it('rejects number input', () => { ... });
```

Good (8 lines for the same concept):
```typescript
it.each([
  ['null', null],
  ['undefined', undefined],
  ['empty string', ''],
  ['whitespace', '  '],
  ['number', 42],
])('rejects invalid input: %s', (_label, input) => {
  expect(validate(input)).toEqual({ success: false });
});
```

Rule: **if 3+ tests share the same assertion pattern with different inputs, use `it.each()`.**

## Mocking Budget

- **Maximum 3 `vi.mock()` calls per test file.** If you need more, you're testing at the wrong level.
- **Never mock the thing you're testing.**
- **Prefer dependency injection over mocking.** If a function takes its dependencies as parameters, test it with real or stub implementations, not `vi.mock()`.

### When to mock vs. test real code

| Situation | Approach |
|-----------|----------|
| Pure function (builder, validator, guard) | **No mocks** — pass inputs, check outputs |
| Function with I/O (database, API, filesystem) | **Mock the I/O boundary only** |
| Hook that calls other hooks | **Test with `renderHook`** from testing-library, mock only external I/O |
| React component | **Render test** with testing-library, mock only API/auth |
| Server action / API route | **Integration test** with real logic, mock only external services |

### Mock setup pattern

Bad (60 lines of boilerplate):
```typescript
const mockFnA = vi.fn();
const mockFnB = vi.fn();
const mockFnC = vi.fn();
// ... 20 more
vi.mock('@/lib/server/foo', () => ({ fnA: (...args) => mockFnA(...args) }));
vi.mock('@/lib/server/bar', () => ({ fnB: (...args) => mockFnB(...args) }));
// ... 10 more vi.mock blocks
```

Good (shared fixture, 5 lines):
```typescript
vi.mock('@/lib/server/externalApi');
const mockApi = vi.mocked(externalApi);
// That's it. Everything else is real code.
```

## Test Structure

Follow Arrange-Act-Assert (AAA):

```typescript
it('calculates total with tax for US customers', () => {
  // Arrange
  const cart = buildCart({ items: [{ price: 100, qty: 2 }], country: 'US' });

  // Act
  const total = calculateTotal(cart);

  // Assert
  expect(total).toBe(216); // 200 + 8% tax
});
```

- **Arrange** should be 1-3 lines. If it's more, extract a builder function.
- **Act** should be exactly 1 line.
- **Assert** should be 1-3 lines. Prefer one assertion per test.

## Naming Convention

Test names should read as a specification:

```typescript
describe('calculateTotal', () => {
  it('returns zero for an empty cart', () => { ... });
  it('applies tax rate based on customer country', () => { ... });
  it('caps discount at 50% of subtotal', () => { ... });
  it('throws when currency is unsupported', () => { ... });
});
```

Pattern: `it('<verb>s <outcome> <condition>')` — present tense, third person.

## What NOT to Test

- **Type correctness** — TypeScript already checks this. Don't write tests that assert a function returns a string.
- **Framework behaviour** — Don't test that React renders, that Zod validates, or that Vitest mocks work.
- **Implementation details** — Don't test internal state shape, mock call counts, or private method sequences.
- **Constants** — Don't test that `MAX_RETRIES === 3`. The constant is the source of truth.

## What TO Test

- **Business logic** — Given these inputs, does the function produce the correct output?
- **Edge cases** — Boundary values, empty collections, off-by-one scenarios.
- **Error paths** — Does the function fail gracefully with invalid input?
- **State transitions** — For stateful code, does the state change correctly?
- **Integration points** — Does the function work with its real dependencies?

## Edge Case Categories (Behavioural, Not Permutational)

Instead of enumerating every possible invalid input, test by **behavioural category**:

1. **Empty/missing** — What happens when nothing is provided?
2. **Boundary** — What happens at exact limits (0, max, threshold)?
3. **Malformed** — What happens with structurally wrong input?
4. **Concurrent** — What happens when called simultaneously?
5. **Error propagation** — What happens when a dependency fails?

One test per category is usually enough. Use `it.each()` within a category if there are meaningful variations.

## Anti-Patterns

- **The Giant Test File** — 1,000+ lines testing one module. Split it.
- **The Permission Test** — Tests that assert broken behaviour is correct (`BUG: accepts path traversal`).
- **The Mirror Test** — Tests that restate the implementation (`expect(fn()).toBe(fn())`).
- **The Mock Orchestra** — 10+ `vi.mock()` calls. You're testing glue code, not behaviour.
- **The Duplicate Suite** — A `.test.ts` and a `.adversarial.test.ts` that overlap 50%.
