# Coding Standards

## General

Prefer clear, idiomatic Rust.

Avoid unnecessary boilerplate.

Prefer simple implementations over clever ones.

Do not introduce abstractions solely in anticipation of hypothetical future
requirements.

Prefer composition over deep trait hierarchies.

Prefer explicit domain types when they prevent invalid states or clarify
ownership.

Keep I/O, terminal emulation, semantic state, application state, and rendering
separated.

## Ownership and Borrowing

Prefer immutability where practical.

Avoid global mutable state.

Prefer ownership and borrowing patterns that keep lifetimes local and obvious.

Avoid explicit lifetime complexity unless it provides a meaningful benefit.

Do not clone data merely to avoid understanding ownership unless the copy is
known to be inexpensive or appropriate.

## Iteration

Use iterator chains when they make transformations clearer.

Use ordinary loops when control flow, mutation, early exits, or state-machine
logic is clearer that way.

Readability takes precedence over avoiding loops.

## Unsafe Code

Application code should contain no `unsafe` unless it is genuinely unavoidable.

Prefer safe abstractions around FFI and platform APIs.

Any unavoidable `unsafe` must:

- be isolated behind a small abstraction;
- have a documented safety invariant;
- explain why a safe alternative is not practical.

Do not spread unsafe assumptions through application code.

## Error and Option Handling

Use `?` to propagate errors or absence when the caller is the appropriate place
to decide how to handle them.

Libraries should expose typed errors when callers can meaningfully react to
different failure modes.

Application boundaries may use contextual/general errors where distinguishing
individual error variants provides no benefit.

Do not silently discard errors.

Avoid `unwrap()` and `expect()` in runtime paths unless an invariant makes
failure genuinely impossible and that invariant is obvious or documented.

## Dependencies

Prefer mature, focused dependencies.

Do not add a dependency for functionality that is trivial to implement
correctly using the standard library.

Do not replace standard-library functionality without a concrete reason.

Large or architecture-defining dependencies should be justified before
adoption.

## Performance

Performance matters, particularly for:

- keyboard input latency;
- PTY processing;
- terminal parsing;
- scrolling;
- text shaping;
- rendering.

Do not optimize based solely on intuition.

Avoid obvious unnecessary allocation or copying in per-frame and PTY hot paths.

Prefer measuring before introducing architectural complexity for performance.

Add benchmarks when a performance-sensitive component becomes stable enough
for them to provide useful regression detection.

## Testing

Test externally meaningful behavior and component boundaries rather than
implementation details.

Prefer integration tests for interactions between major components.

Use unit tests for:

- deterministic domain logic;
- parsers;
- state transitions;
- protocol handling;
- edge cases.

Terminal and semantic protocol behavior should be testable using recorded or
constructed byte streams where practical.

Tests should not unnecessarily require a graphical environment or interactive
shell.

## Formatting and Linting

Code should pass:

    cargo fmt --check
    cargo clippy --all-targets
    cargo test

Warnings should generally be resolved rather than suppressed.

Lint suppression should be narrow and justified.
