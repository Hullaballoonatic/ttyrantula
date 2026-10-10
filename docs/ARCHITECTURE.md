# Architecture

## Status

This architecture describes current intended boundaries.

Implementation experience may reveal better boundaries or invalidate technical
assumptions. Such discoveries should result in discussion and documentation
changes rather than workarounds that violate these boundaries.

## Principles

Terminal correctness and semantic presentation are separate concerns.

Loss or absence of semantic shell integration must never prevent ordinary
terminal operation.

The terminal emulation library owns VT semantics. Application code must not
independently reimplement VT parsing.

Semantic command state is application state and should remain independent of
terminal rendering state.

Presentation state should remain independent of persistent command data.

## High-Level Data Flow

Conceptually:

    Shell / Child Process
            │
            │ PTY byte stream
            ▼
       PTY Session
            │
            ▼
       Terminal Core
          │     │
          │     └──── semantic events ────┐
          │                               ▼
          │                         Semantic Layer
          │                               │
          │                               ▼
          │                         Command Model
          │                               │
          └──────────────┬────────────────┘
                         ▼
                    Application State
                         │
                         ▼
                       Renderer
                         │
                         ▼
                     GPU / Window

The exact implementation of this flow may differ, but these conceptual
responsibilities should remain separated.

## PTY Session

The PTY Session owns communication with the child shell or process.

Responsibilities include:

- spawning the shell;
- reading terminal output;
- writing terminal input;
- resizing the pseudoterminal;
- observing process lifecycle.

PTY handling should not contain command-presentation logic.

The current trial uses pinned `portable-pty` 0.9.0 behind a small local
`PtySession` boundary. It owns spawning, byte transport, resize, and child
lifecycle without exposing portable-pty types to the rest of the application.
The boundary is independently tested with a real shell process. A
`TerminalSession` provides the synchronous proof boundary, while the threaded
worker path separates a blocking `PtyReader` from control operations, keeps
Ghostty on one owner thread, and publishes latest-frame snapshots for GUI
consumers. Renderer wiring remains a later step.

## Terminal Core

The application intends to use `libghostty-vt` rather than implementing VT
emulation itself.

The Terminal Core is responsible for terminal semantics including, as provided
by the library:

- VT parsing;
- terminal grid state;
- cursor state;
- terminal modes;
- scrollback;
- resize/reflow behavior;
- alternate-screen state;
- Unicode/grapheme-related terminal behavior.

Application code should not duplicate these responsibilities.

The exact Rust integration strategy for `libghostty-vt` should be validated
before committing heavily to it.

### Terminal Core Trial

The initial integration trial uses `libghostty-vt` Rust wrapper 0.2.2 with
Ghostty revision `a887df42c56f6de86c0fe6da9c4eeca37931e083`. Nix builds that exact
Ghostty revision and exposes it to the wrapper through `pkg-config`, avoiding a
network fetch from the Cargo build script. The raw bindings dependency is
listed directly only to enable its `pkg-config` feature; terminal code uses the
safe wrapper.

All wrapper objects are contained in the local `terminal` module. Its public
boundary consists of owned application types so Ghostty lifetimes, FFI types,
and unstable APIs do not leak into the rest of the application.

At this revision, OSC 133 produces prompt, input, and output classifications
on terminal cells. Neither the public C API nor the Rust wrapper exposes
command-started or command-completed callbacks. The trial therefore preserves
cell classifications but does not infer application command lifecycle events.
How lifecycle boundaries should be obtained remains an open question.

Until an upstream OSC dispatch API is available, a small local streaming
observer may decode only OSC 133 lifecycle markers. It must remain separate
from VT parsing, tolerate PTY chunk boundaries, and be replaceable when the
upstream API changes. It must not grow into a second terminal parser.

The Ghostty revision, Rust wrapper version, and Nix inputs should be treated as
an explicit compatibility set. Upgrades should be deliberate: review upstream
`libghostty-vt` API changes, update the pinned set together, rerun recorded
terminal and semantic-stream tests, and document any adapter changes.

## Semantic Layer

The Semantic Layer interprets terminal/shell integration information relevant
to semantic commands.

Potential information includes:

- prompt boundaries;
- command input boundaries;
- command execution start;
- command completion;
- exit status;
- working-directory changes.

OSC 133 and related shell-integration protocols are expected to be important
inputs.

The Semantic Layer translates protocol-level events into application-level
state changes.

Protocol details should not leak unnecessarily into the Command Model or UI.

When semantic information is unavailable or incomplete, the Semantic Layer
must degrade gracefully rather than compromising ordinary terminal behavior.

## Command Model

The Command Model represents semantic command history independently of how that
history is drawn.

Conceptually:

    Command
    ├── Input
    ├── Output
    ├── Metadata
    └── Lifecycle

Lifecycle may include states such as:

- Running
- Completed

Presentation concerns such as:

- hover
- selection
- summarized/expanded state
- border appearance

do not belong to the persistent Command model.

Exact Rust types should emerge from implementation rather than being fixed by
this document prematurely.

## Application State

Application State coordinates:

- PTY/session state;
- terminal state;
- semantic Command state;
- Composer state;
- UI state;
- focus and transient pointer state.

Avoid global mutable state.

The application should have an understandable owner for each mutable piece of
state.

## Source Modules

Source modules follow architectural responsibilities rather than mapping one
file to every type. The executable entry point remains thin, while application
event coordination lives in `app`. Domain state is divided into focused
`model` modules, and graphical presentation is divided into `ui` geometry,
state, scene construction, and Command presentation modules. Platform adapters
such as the clipboard and rendering backend remain isolated from those areas.

PTY, terminal-core, and semantic modules should be introduced when those
components exist rather than as empty placeholders.

## Input Routing

Window-system keyboard events are normalized before the application interprets
them. A centralized, context-aware keymap translates key chords into
application actions. Printable Composer text remains a separate input path so
changing a navigation binding does not affect text decoding.

Pointer and keyboard activation dispatch the same Command actions. The current
prototype edits mock Composer text directly; after PTY integration, ordinary
terminal input belongs to the PTY and Terminal Core path rather than a custom
line editor. Alternate-screen applications must receive conventional terminal
input without semantic-history navigation interfering.

## Clipboard

Clipboard formatting is a pure transformation of Command data and copy scope.
Platform clipboard ownership is isolated from the Command Model and renderer.

The initial Wayland implementation uses `smithay-clipboard`, which is designed
for windowed Wayland applications and shares the Smithay client stack already
used by `winit`. Its required raw Wayland display conversion is contained in a
small adapter that retains the originating window for the adapter's lifetime.
Clipboard requests are serialized on a dedicated worker so reads cannot block
the window event loop. Completed reads return through a `winit` user event;
only the GUI thread updates Composer state.

## Renderer

The Renderer turns terminal and application state into graphical output.

It is responsible for presentation such as:

- terminal glyphs;
- terminal backgrounds;
- command backgrounds;
- command borders;
- Composer presentation;
- metadata;
- hover and focus treatments;
- running-state indicators.

Command borders and backgrounds are graphical primitives and must never become
terminal cells.

The rendering architecture should prioritize:

- low input latency;
- smooth scrolling;
- efficient incremental updates;
- predictable frame times;
- accurate text rendering.

A custom GPU-rendered surface based on `winit` and `wgpu` is a leading
candidate.

Text rendering/shaping solutions such as `cosmic-text` and `glyphon` should be
evaluated rather than implementing text shaping from scratch.

## Alternate Screen

When the terminal enters the alternate screen, the application should render
the terminal conventionally.

Semantic history and the Composer are hidden for the duration of the
alternate-screen session.

When the alternate screen exits, the normal semantic shell UI is restored.

This provides a correctness-first fallback for TUIs and multiplexers.

## Concurrency

No concurrency model is mandated yet.

Do not introduce an async runtime solely because PTY I/O is asynchronous in
nature.

A small number of threads and channels may be preferable to an async runtime
for the initial implementation.

Choose a concurrency model based on demonstrated requirements.

## Candidate Dependencies

Current candidates include:

| Responsibility | Candidate |
| --- | --- |
| VT emulation | `libghostty-vt` |
| PTY abstraction | `portable-pty` |
| Window/event handling | `winit` |
| GPU rendering | `wgpu` |
| Text shaping | `cosmic-text` |
| GPU glyph rendering | `glyphon` |
| Wayland clipboard | `smithay-clipboard` |
| Application-level errors | `anyhow` |
| Typed errors | `thiserror` |
| Diagnostics/tracing | `tracing` |
| Serialization, when needed | `serde` |

These are candidates rather than permanent requirements.

Before adopting one, confirm that its current API, maintenance status,
licensing, and architecture are appropriate.

Avoid adding an async runtime, configuration framework, CLI framework,
database, ECS, or similar infrastructure until a concrete requirement exists.

## Initial Development Strategy

Do not attempt to implement the complete terminal immediately.

Prefer independently proving the major architectural pieces.

A useful progression is:

1. Render a window containing hard-coded mock Command data.
2. Prove the desired command-block layout and resizing behavior.
3. Establish PTY communication with an interactive shell.
4. Integrate the terminal emulation core and render an ordinary terminal.
5. Detect semantic shell-integration events.
6. Build Command objects from those events.
7. Integrate semantic Commands with the graphical presentation.
8. Validate alternate-screen behavior.
9. Investigate multiplexer behavior and deeper integration separately.

Early prototypes may be discarded if they successfully answer architectural or
UI questions.
