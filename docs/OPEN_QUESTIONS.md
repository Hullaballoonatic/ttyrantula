# Open Questions

This document records questions that are intentionally unresolved.

Do not treat an item here as a requirement merely because one possible design
is mentioned.

Implementation and prototyping should provide evidence for resolving these
questions.

## UI

- Should Nushell structured tables have a native GUI presentation? If so,
  how should structured data reach the renderer while preserving conventional
  terminal output, copying, and behavior across different shells?

- What exact visual distinction should exist between Input and Output?
- Should Command blocks have rounded corners?
- What spacing should exist between historical Commands?
- How prominent should borders be?
- What metadata belongs in the Composer?
- What metadata should normally be visible on historical Commands?
- Should detailed metadata appear on hover, expansion, or some combination?
- How should successful and failed Commands differ visually?
- What animation, if any, should indicate a Running Command?
- How should very large outputs be represented?
- Should Output have explicit collapsed/expanded states?
- How should a Running Command be represented while another historical Command
  is expanded?
- What should happen visually when the Composer contains partially entered
  input?

## Command History

- Should semantic command history survive application restarts?
- If persisted, should Output also persist?
- Should history exist independently from shell history?
- What limits should apply to retained Output?
- How should commands producing enormous or continuous output behave?
- How should commands that redraw existing terminal rows be represented?

## Metadata

- Which metadata should be captured for every Command?
- At what lifecycle point should each metadata field be snapshotted?
- Should Git metadata come from shell integration, asynchronous application
  inspection, or another mechanism?
- How should remote SSH sessions affect metadata collection?
- Should users eventually be able to define custom metadata?

## Terminal Semantics

- How should Atuin integrate with the Composer, suggestions, command history,
  and its interactive history interface? Compatibility remains unproven;
  personal Atuin configuration is currently excluded from development sessions.

- How should semantic Commands interact with applications that do not use the
  alternate screen but nevertheless perform complex cursor manipulation?
- How should partially supported OSC 133 shell integration degrade?
- Which shells should receive first-class integration beyond Nushell?
- How much shell-specific integration should the project own?

## Multiplexers

- Which Herdr layout and workflow features should form the initial built-in
  multiplexing baseline, and which should be deferred?
- How should the sidebar represent ongoing processes and agent states?
- Which reliable signals or explicit integrations can report agent state,
  including unavailable or unknown state?
- What session/pane switching, splitting, and lifecycle behavior is required
  for the first dogfooding milestone?

- Can OSC 133 and related semantic information be preserved reliably through
  tmux, Zellij, and herdr?
- Should multiplexers initially receive only conventional terminal rendering?
- Is it practical for the application to understand multiplexer pane
  boundaries?
- Would deep multiplexer integration require explicit cooperation from the
  multiplexer?
- How should alternate-screen fallback interact with multiplexers?

## Architecture

- What is the best current Rust integration for `libghostty-vt`?
- Given that the pinned `libghostty-vt` C API and Rust wrapper expose OSC 133
  cell classifications but not command lifecycle callbacks, where should
  command-started and command-completed events come from?
- Is `portable-pty` the appropriate PTY abstraction?
- Is `winit` + `wgpu` the appropriate long-term rendering stack?
- Should `cosmic-text`, `glyphon`, another text stack, or functionality exposed
  by Ghostty be used for text rendering?
- What concurrency model best fits PTY I/O and the GUI event loop?
- Which boundaries need their own crates versus ordinary Rust modules?
- At what point, if any, would splitting the workspace into multiple crates
  improve maintainability?

## Platform Scope

- Linux/Wayland is the initial development target.
- How early should X11 support be considered?
- At what stage should macOS support become a requirement?
- At what stage should Windows support become a requirement?
- Which platform abstractions should be adopted now to avoid needless
  platform-specific coupling without prematurely implementing other platforms?
