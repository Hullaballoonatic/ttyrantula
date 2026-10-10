# Project Instructions

This is an experimental graphical terminal emulator written in Rust.

The defining feature of the application is semantic command presentation:
ordinary shell commands and their output are presented as distinct graphical
command blocks, while full-screen terminal applications continue to behave
like an ordinary terminal.

## Documentation

Read the relevant documentation before making changes:

- `docs/PRODUCT.md` — product goals, scope, and terminology
- `docs/UI.md` — visual behavior and interaction design
- `docs/ARCHITECTURE.md` — architectural boundaries and technical decisions
- `docs/STANDARDS.md` — Rust and project coding standards
- `docs/OPEN-QUESTIONS.md` — intentionally unresolved decisions

Product requirements and architectural decisions should be reflected in these
documents rather than existing only in implementation details.

Documentation describes current intent, not unquestionable implementation
truth. If implementation work reveals that a documented assumption is
incorrect, unnecessarily difficult, or conflicts with another requirement,
stop and explain the conflict rather than silently working around it.

Do not silently resolve questions listed in `OPEN-QUESTIONS.md`.

## Priorities

In order:

1. Correct terminal behavior
2. Low input latency and smooth rendering
3. Simple, understandable architecture
4. Visual polish
5. Additional features

Correct ordinary terminal behavior must not depend on semantic shell
integration being available.

## Scope

Do not implement unrelated terminal features merely because other terminals
provide them.

In particular, do not expand the application toward an IDE, shell, multiplexer,
AI assistant, or general-purpose productivity environment unless the product
documentation explicitly changes to include such functionality.

Prefer focused solutions to speculative extensibility.

## Development

Prefer small, independently testable components.

Keep terminal emulation, PTY I/O, semantic command state, and graphical
presentation separated.

Do not reimplement functionality that belongs to the terminal emulation
library.

Before adding a major dependency:

1. determine whether an existing dependency or the standard library already
   provides the required functionality;
2. explain why the dependency is appropriate;
3. consider its maintenance status, scope, and effect on the architecture.

Do not introduce abstractions solely for hypothetical future requirements.

## Validation

After changes, run:

- `cargo fmt --check`
- `cargo clippy --all-targets`
- `cargo test`

Fix failures introduced by the change before considering the work complete.
