# UI

## Principles

The UI should make command boundaries immediately apparent without requiring
visual clutter.

Information should appear when useful rather than being permanently repeated.

The terminal should remain visually calm when idle.

Graphical decoration must remain independent of terminal contents.

## Overall Layout

During ordinary shell interaction, the surface consists conceptually of:

    ┌─────────────────────────────────────────────┐
    │                                             │
    │             Command History                 │
    │                                             │
    │ ┌─────────────────────────────────────────┐ │
    │ │ Most recent / running command           │ │
    │ └─────────────────────────────────────────┘ │
    │                                             │
    ├─────────────────────────────────────────────┤
    │ Composer                                    │
    └─────────────────────────────────────────────┘

The Composer remains at the bottom.

Commands extend upward into the past.

The newest Command appears directly above the Composer.

## Composer

The Composer contains the active shell input.

It should also expose the most useful current contextual metadata without
requiring that information to be repeated in every shell prompt.

Potential metadata includes:

- current working directory
- Git branch
- other current shell context determined to be useful

The exact arrangement is intentionally unresolved.

The Composer should visually belong to the application rather than appearing
to be another historical Command.

The prototype accepts ordinary clipboard paste into the Composer with
`Ctrl+V`. Pasted text follows the same single-line control-character filtering
as typed text until terminal input is connected to the PTY.

## Command Blocks

Each semantic Command is visually represented as a block.

A block may contain:

- Input
- Output
- Metadata

The block itself has a graphical boundary.

Input and Output should primarily be distinguished through graphical styling,
such as different background intensity or transparency, rather than additional
nested borders.

For example:

    ┌─────────────────────────────────────────────┐
    │ metadata                                    │
    │ ❯ cargo test                                │  Input
    │                                             │
    │ running 27 tests                            │  Output
    │ test result: ok                             │
    └─────────────────────────────────────────────┘

The actual appearance is not required to resemble terminal box-drawing
characters.

## Presentation States

Presentation state is independent of Command lifecycle.

### Summary

A summarized Command prioritizes identifying the command while consuming little
vertical space.

Conceptually:

    cargo test                              18:42

The Input is left-aligned and limited to a single displayed line.

Execution time or another compact useful indicator may appear on the right. All
such right-side status text shares the Command's right content edge.

When space is constrained, right-side metadata takes precedence and Input may
be truncated.

Completion status may be represented through an icon, border styling,
background styling, or another unobtrusive visual treatment.

### Expanded

An expanded Command may show:

- full Input
- Output
- Metadata

Expansion should make it possible to inspect a Command without permanently
giving every Command the same visual weight.

### Command Actions

While using a mouse, hovering over a Command's header reveals a compact set of
icon buttons for copying all content, rerunning, and expanding or collapsing
that Command. The buttons are hidden when the Command is not hovered. The
action set overlays the Command's contents rather than reserving permanent
space while hidden.

For an expanded Command, hovering over Input or Output reveals a single copy
button scoped to that section. This section action replaces the main Command
action set so only one floating action set is visible at a time.

Expanding and collapsing is controlled by its disclosure button rather than by
clicking the Command body. Each Command retains its own expansion state until
the user changes it.

Keyboard focus begins in the Composer. The prototype's `Ctrl+H` binding moves
focus to the newest Command. While history has focus, Up/Down or `j`/`k` move
between Commands, Space toggles expansion, Enter reruns, `yy` copies all, `yi`
copies Input, and `yo` copies Output. Escape returns focus to the Composer.

The bindings are provisional and declared together rather than embedded in
event handling. Keyboard and pointer activation invoke the same underlying
Command actions.

A keyboard-focused Command displays its action set and a visible focus
treatment. Pointer movement restores mouse-oriented presentation, in which the
action set is only visible on hover. Moving keyboard focus scrolls the focused
Command into view.

Copy operations produce plain text without graphical prompt symbols. Input and
Output copy their respective content. Copy-all includes labeled working
directory and branch metadata, followed by Input and Output as blank-line
separated sections.

### Running

A Running Command should make ongoing execution apparent.

Its Output is updated as terminal output arrives.

The running state may use subtle animation or another indicator of activity.

When screen space is limited, a Running Command may use a compact presentation
while another historical Command is expanded.

The running Command's Input may become more visually prominent when doing so
helps distinguish it from input currently being composed.

## Metadata

Metadata should generally be secondary to Input and Output.

Detailed metadata may become visible when a Command is:

- hovered
- expanded

The exact interaction remains intentionally unresolved.

Metadata should not cause repeated visual noise across every historical
Command.

## Resizing

Graphical command boundaries must adapt to the current viewport.

Borders and backgrounds are UI geometry rather than terminal characters, so
resizing must not produce wrapped or stale separators.

Terminal contents should reflow according to the behavior of the underlying
terminal engine.

## Full-Screen / Alternate Screen

While the alternate screen is active:

- hide the Composer;
- hide semantic command-history presentation;
- render the application as a conventional terminal surface.

This mode is intended for applications such as Neovim, Yazi, less, htop, and
terminal multiplexers when deeper integration is unavailable.

## Visual Details

The following are deliberately not yet fixed:

- exact colors
- border thickness
- corner radius
- spacing between Commands
- typography
- animation
- hover behavior
- metadata placement

These should be evaluated through prototypes rather than prematurely treated
as architectural requirements.
