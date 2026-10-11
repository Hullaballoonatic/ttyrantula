# Project Handoff

Updated: 2026-10-11

## Current State

User-verified milestone: the Composer, Nushell command execution, live command
history, and hint presentation work on the development machine.

### Follow-up to-dos

- Low priority: implement Stylix support for application theming.

- Very low priority: add playful tarantula-themed visual flair, such as a
  subtle spider-web background. Keep text legible and the terminal visually
  calm; this is an aesthetic motif, not a biological accuracy requirement.

- Dogfooding prerequisites: reliable full-screen handoff for Neovim and
  terminal-based Codex, plus built-in multiplexing inspired by Herdr.
- Add a sidebar showing sessions, ongoing processes, and agent state;
  determine the source of agent-state information before implementing it.
- Make header clicks toggle Command expansion and header right-clicks open
  a context menu for copying and other actions.
- Investigate Atuin compatibility (see `OPEN_QUESTIONS.md`).

- Support selecting and running different shells, with appropriate semantic
  integration and fallback behavior for each.
- Track the active shell, including changes within an existing session, and
  display its name before the current working directory in the Composer.
- Investigate GUI presentation of Nushell's structured tables; this is an
  open design question, not an implementation commitment.

Ttyrantula is an experimental graphical terminal emulator written in Rust for
Linux/Wayland. Its defining feature is semantic command presentation: ordinary
shell commands and their output become graphical Command blocks, while
full-screen applications behave like a conventional terminal.

The application runs Nushell through a threaded PTY/Ghostty runtime. Composer
input and hints, command output/history, exit status, and working-directory
updates are connected to the GUI. Mock commands exist only as test fixtures.
The obsolete synchronous trial coordinator has been removed.

Implemented prototype behavior includes:

- Command summary and expanded presentations;
- independent expand/collapse state;
- hover actions for expand/collapse, copy-all, and rerun;
- section-specific copy actions for expanded Input and Output;
- Wayland system clipboard copy and Composer paste;
- keyboard focus and navigation between the Composer and history;
- provisional centralized keybindings;
- copy-all, copy-input, and copy-output keyboard sequences;
- responsive graphical layout and right-aligned Command metadata.

The source was refactored into focused `model` and `ui` modules. The last full
validation completed successfully with:

- `cargo fmt --check`
- `cargo clippy --all-targets`
- `cargo test`

The terminal-core trial now also includes:

- a Nix-pinned Ghostty revision and `libghostty-vt` package;
- safe Rust wrapper 0.2.2 linked to the Nix-built library through
  `pkg-config`;
- an owned-snapshot `terminal` adapter that contains Ghostty-specific types;
- recorded-stream tests for text, RGB color, cursor motion, Unicode, wide
  cells, resize/reflow, alternate-screen behavior, graceful non-semantic
  operation, and OSC 133 cell classifications.
- a chunk-boundary-safe OSC 133 semantic decoder that can be replaced by an
  upstream dispatch callback when that API becomes available.
- a pinned `portable-pty` 0.9.0 `PtySession` boundary with real-shell tests for
  byte transport, resize, and child exit status.
- a terminal-core worker that routes PTY output through `TerminalCore` and
  exposes owned snapshots and semantic lifecycle events.

## Agreed Direction

The next architectural problem is selecting and proving the underlying
terminal stack. The current recommendation is:

- `libghostty-vt` for VT parsing, terminal grids, cursor and mode state,
  scrollback, resize/reflow, alternate-screen behavior, input encoding, and
  semantic OSC 133 information;
- `portable-pty` for spawning the shell, transporting input/output bytes,
  resizing the PTY, and observing child-process lifecycle;
- ttyrantula's existing `winit`, `wgpu`, and `glyphon` stack for the graphical
  application and renderer.

`libghostty-vt` does not provide a PTY, window, or renderer. `portable-pty`
does not interpret terminal escape sequences. They are complementary.

The recommendation is provisional because the `libghostty-vt` embedding API
is not stable, its safe Rust wrapper is third-party, and its Nix build requires
pinned Ghostty and Zig inputs. The dependency should be isolated behind a
small local adapter. Keep a constant watch on the upstream OSC-dispatch work;
major or breaking releases should trigger a deliberate compatibility review
rather than an automatic version bump.

## Proposed Trial

The original trial has reached live semantic history; remaining limitations
are tracked below. Current progress:

1. [x] Add a reproducible Nix build for a pinned `libghostty-vt` revision.
2. [x] Add a narrow `terminal` adapter and recorded-stream tests.
3. [x] Verify text, colors, cursor movement, Unicode, resize/reflow,
   alternate-screen state, OSC 133 classifications, and lifecycle API
   availability. Lifecycle callbacks are absent in the selected API, so the
   local OSC 133 decoder remains provisional.
4. [x] Route PTY output through the terminal core and input bytes back to the
   PTY through the terminal-core worker.
5. [x] Establish non-blocking I/O scheduling so PTY reads cannot stall the
   window loop. The PTY boundary is now split into a blocking reader worker
   and nonblocking control operations. A terminal-core worker now owns
   Ghostty, consumes that channel, and publishes snapshots/events through a
   latest-frame mailbox.
6. [~] Render a conventional live terminal in the existing Wayland
   application. The application now starts the shell worker, polls the latest
   snapshot during `about_to_wait`, and renders terminal rows in the history
   viewport. Input and resize are connected, but accurate viewport/font
   geometry and complete full-screen rendering/input remain unfinished.
7. [x] Translate semantic lifecycle events into Running and Completed Commands.
8. [x] Replace application mock history with live semantic history.

The next milestone is dogfooding readiness: preserve long output, finish
full-screen handoff, and establish built-in multiplexing.

## Intended Runtime Boundaries

### Next semantic integration checkpoint

Cleanup removed the synchronous `TerminalSession`, duplicate direct PTY
operations, and local Composer editing fallback. Shell startup failures are
shown in the GUI. Rerun replaces the editable line and submits it in the
current shell directory once Nushell confirms the replacement; running
commands cannot be rerun concurrently. Multiline rerun remains unsupported.

Hints are now enabled with a controlled dim style. Composer input and hint
suffix are separate fields, drawn as normal text over a gray suggestion.
Right Arrow accepts hints through Nushell's default binding. History uses an
isolated temporary configuration directory per session so commands entered
in this session can provide hints without loading personal history or Atuin.
The real Nushell regression verifies hint separation, Right Arrow acceptance,
and execution of the accepted command.

Composer projection preserves trailing spaces up to the shell cursor.
OSC 7 updates Composer working-directory metadata and the directory
recorded on subsequent commands; historical entries retain their execution
directory. Real Nushell regressions cover `l`, trailing spaces, deletion, and
changing directory; protocol tests cover percent-encoded directory paths.

Live command history now replaces the application mock entries. The worker
observes OSC 133 in byte order, emits command-start/output/completion events,
and uses a separate Ghostty core to render each command's output as plain
text. The GUI creates Running entries and completes them with exit status.
A real Nushell test verifies command text, output, and successful completion.
Current limits: captured output is limited to the command capture viewport
(scrollback capture is still required); durations are not recorded;
output styles are plain text. Do not treat the earlier checklist as complete
terminal behavior. These limitations remain follow-up work.

During development, Nushell starts with `--no-config-file` and an isolated
configuration directory to avoid personal startup hooks (including Atuin)
and personal history. No user
configuration files are changed. The real Nushell regression checks exact
Composer text after individual letters and Backspace. Application keybindings
now run before PTY input routing, and history focus suppresses shell typing.

The application now launches `nu` instead of `sh`. The Composer projects
shell-owned input cells from the active cursor row using Ghostty's OSC 133
classifications; typing and editing continue to go to Nushell. Integrated
prompt/input rows are no longer overlaid on history as a conventional shell.
The conventional view is currently used for alternate-screen applications.
Paste is forwarded as bracketed PTY input. Projection handles single-row
input only: multiline input, completion menus, and accurate cursor rendering
remain unfinished.

Fixed the blank Nushell Composer by returning Ghostty-generated terminal
protocol responses to the PTY. Nushell's startup queries previously received
no response. A real Nushell regression test now types a command without
submitting it and verifies its text reaches the Composer projection.

Use Nushell for the initial semantic shell integration, as agreed with the
user. Nushell is installed locally; choosing other shells can follow later.
The conventional shell surface is a temporary proof and fallback, not the
intended ordinary-command presentation. Present shell-owned editable input
in the Composer and translate shell lifecycle/output information into live
Command history. Preserve shell editing, completion, and history behavior.
Correct resize geometry to use the actual terminal viewport and font cell
metrics rather than the whole window and guessed dimensions.

Do not consider step 6 complete until this presentation path is verified.

The safe Rust wrapper's Ghostty objects are not `Send` or `Sync`. The proposed
initial concurrency model is:

- the existing GUI thread;
- one blocking PTY reader thread;
- one terminal-core thread that owns all Ghostty objects;
- channels between these components;
- a replaceable latest-frame mailbox so heavy output cannot flood the GUI
  event loop.

The Semantic Layer should use a focused, mostly pure reducer that translates
protocol events into application-level Command changes. Ghostty types and OSC
details should not leak into the Command model or UI.

## Important Behavioral Decision

Once a real shell is connected, the Composer must not remain an independent
line editor. Keyboard input must go immediately to the PTY so shell editing,
completion, history, and terminal applications behave correctly. The Composer
should graphically present the active shell prompt/input derived from terminal
state.

When semantic shell integration is missing or incomplete, ttyrantula must
remain usable by rendering the conventional terminal surface.

## Still Unresolved

Do not silently decide these questions during implementation:

- whether ttyrantula bundles, adapts, or independently implements shell
  integration scripts;
- which shells receive first-class support;
- how to obtain semantic command lifecycle events, because the selected C API
  and Rust wrapper expose OSC 133 cell classifications but not command
  lifecycle callbacks. A local narrow decoder is currently the provisional
  bridge, pending upstream dispatch support;
- whether `portable-pty` remains the right abstraction after the PTY trial;
- the long-term persistence and size limits for command history and output;
- behavior for programs that manipulate the primary screen without using the
  alternate screen.

See `docs/PRODUCT.md`, `docs/UI.md`, `docs/ARCHITECTURE.md`,
`docs/STANDARDS.md`, and `docs/OPEN_QUESTIONS.md` before making changes.
