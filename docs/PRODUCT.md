# Product

## Vision

The application is a fast, minimal graphical terminal emulator whose command
history is presented as structured graphical objects rather than an
undifferentiated stream of terminal rows.

It should feel approximately like Ghostty in scope and responsiveness, while
borrowing the visual concept of command blocks from Warp.

The objective is not to reproduce Warp's feature set. The attraction is its
visual distinction between commands.

The application should remain recognizably a terminal emulator.

## Non-Goals

The application is not intended to provide:

- AI features
- cloud accounts or cloud synchronization
- shell command history search
- IDE functionality
- file browsers
- notebooks
- a replacement shell

These are explicitly outside the current product vision.

## Core Interaction

Ordinary shell interaction is represented using semantic commands.

A command conceptually contains:

- Input
- Output
- Metadata
- Lifecycle state

For example:

    ┌─────────────────────────────────────────────────────────────┐
    │ ~/projects/foo                                      main    │
    ├─────────────────────────────────────────────────────────────┤
    │ ❯ cargo test                                                │
    ├─────────────────────────────────────────────────────────────┤
    │ running 27 tests                                            │
    │ test result: ok                                             │
    └─────────────────────────────────────────────────────────────┘

The borders, backgrounds, metadata, and other decorations are graphical UI.
They are not terminal characters and must not become part of the PTY byte
stream or terminal grid.

## Terminology

### Composer

The Composer is the persistent area in which the user interacts with the
current shell prompt and enters the next command.

The Composer is not itself a historical Command.

It may display current contextual metadata such as the working directory or
Git branch.

### Command

A Command represents one execution initiated through the shell.

It may contain:

- the submitted input;
- terminal output associated with that execution;
- metadata captured during its lifecycle;
- execution state.

### Running Command

A Command that has begun execution and has not yet completed.

Output may be streamed into it as execution proceeds.

### Completed Command

A Command whose execution has finished.

Completed commands become part of command history.

### History

The ordered collection of completed Commands.

Historical Commands may have different presentation states, such as summarized
or expanded, without changing the underlying Command data.

## Command Lifecycle

Conceptually:

    Composer
       │
       │ submit
       ▼
    Running Command
       │
       │ completion
       ▼
    Completed Command
       │
       ▼
    History

Presentation state is independent of this lifecycle.

For example, a Completed Command may be summarized or expanded.

## Metadata

Useful command metadata may include:

- working directory
- Git branch
- exit status
- duration
- execution/start time

Metadata should be unobtrusive unless it is actively useful.

Metadata associated with a Command should represent the relevant state at the
time the Command was executed rather than being reconstructed from current
state later.

Metadata may be obtained through shell integration, terminal protocols, or
other application-observable state as appropriate.

The exact metadata acquisition mechanisms are architectural concerns rather
than product requirements.

## Graceful Degradation

Semantic command presentation is an enhancement to terminal behavior.

If semantic shell integration is missing, unsupported, incomplete, or
temporarily unavailable, the application must remain usable as an ordinary
terminal emulator.

Terminal correctness takes precedence over command-block presentation.

## Full-Screen Applications

Full-screen terminal applications such as:

- Neovim
- Yazi
- less
- htop

should behave as conventional terminal applications.

When an application uses the alternate screen, the semantic command history and
Composer are hidden and the terminal surface is rendered conventionally.

Leaving the alternate screen restores the semantic shell presentation.

The application should not attempt to represent the internal activity of a
full-screen TUI as command blocks.

## Multiplexers

Terminal multiplexers such as:

- tmux
- Zellij
- herdr

must remain usable as ordinary terminal applications.

Semantic command presentation inside multiplexer panes is desirable, but is
not an MVP requirement.

The architecture should avoid unnecessarily preventing deeper multiplexer
integration in the future.

Built-in multiplexing is now part of the product direction and a prerequisite
for dogfooding. The application will own session and pane management so each
terminal session can retain its own semantic Composer and command history.
Herdr is the initial layout and workflow reference, not a required runtime
dependency or a commitment to reproduce every feature.

The sidebar should show terminal sessions, ongoing processes, and the state of
terminal-based agents. This visibility does not imply an embedded AI assistant
or agent orchestration. How agent state is obtained remains unresolved.

Deeper integration with existing multiplexers remains an open possibility if
their interfaces prove suitable for the application's graphical layout.
