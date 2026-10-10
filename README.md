# ttyrantula

An experimental graphical terminal emulator that presents ordinary shell
commands and their output as distinct graphical blocks.

The current executable is an interactive rendering prototype backed by mock
command data. It exists to evaluate command hierarchy, resizing, disclosure,
scrolling, keyboard navigation, and the persistent Composer before PTY and
terminal emulation are integrated.

## Run

On Linux with a working Vulkan or OpenGL graphics stack:

```sh
cargo run
```

Hover over a command to reveal its action buttons. Copy-all and disclosure are
functional; rerun remains disabled until the shell-session boundary exists.
On an expanded command, hovering over Input or Output replaces the main action
set with a copy button scoped to that section. Scroll over command history to
inspect commands outside the visible region.

The Composer accepts basic text entry, Backspace, and system-clipboard paste
with `Ctrl+V`. `Ctrl+H` moves keyboard focus to the newest Command, Up/Down or
`j`/`k` move through history, Space toggles expansion, and Escape returns to
the Composer. Enter dispatches the currently inactive rerun action. `yy`,
`yi`, and `yo` copy the focused Command's full presentation, Input, and Output
respectively. These prototype bindings are declared together in
`src/input.rs`.
