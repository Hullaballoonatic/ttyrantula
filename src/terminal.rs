//! Narrow adapter around the unstable `libghostty-vt` embedding API.
//!
//! Ghostty handles remain private to this module. Consumers receive owned
//! snapshots so terminal-core lifetimes and FFI types cannot leak into the
//! application model or renderer.

use std::{error::Error, fmt};

use libghostty_vt::{
    RenderState, Terminal, TerminalOptions,
    render::{CellIteration, CellIterator, CursorViewport, RowIterator},
    screen::{CellSemanticContent, CellWide, Screen},
    style::{RgbColor as GhosttyRgbColor, Underline as GhosttyUnderline},
};

const DEFAULT_SCROLLBACK_ROWS: usize = 10_000;

/// Dimensions of the terminal grid in cells.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct GridSize {
    pub columns: u16,
    pub rows: u16,
}

impl GridSize {
    /// Creates non-empty grid dimensions.
    pub fn new(columns: u16, rows: u16) -> Result<Self, TerminalError> {
        (columns > 0 && rows > 0)
            .then_some(Self { columns, rows })
            .ok_or(TerminalError::InvalidGridSize { columns, rows })
    }
}

/// The terminal screen buffer currently being displayed.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ScreenKind {
    Primary,
    Alternate,
}

/// An RGB color independent of the terminal library's representation.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RgbColor {
    pub red: u8,
    pub green: u8,
    pub blue: u8,
}

/// Underline presentation for a terminal cell.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Underline {
    None,
    Single,
    Double,
    Curly,
    Dotted,
    Dashed,
    Unknown,
}

/// Resolved visual attributes for one terminal cell.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CellStyle {
    pub foreground: Option<RgbColor>,
    pub background: Option<RgbColor>,
    pub bold: bool,
    pub italic: bool,
    pub faint: bool,
    pub blink: bool,
    pub inverse: bool,
    pub invisible: bool,
    pub strikethrough: bool,
    pub overline: bool,
    pub underline: Underline,
}

/// How a cell participates in a wide grapheme.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CellWidth {
    Narrow,
    Wide,
    SpacerTail,
    SpacerHead,
}

/// Semantic classification assigned by OSC 133 processing.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SemanticContent {
    Output,
    Input,
    Prompt,
}

/// One owned terminal grid cell.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Cell {
    pub text: String,
    pub width: CellWidth,
    pub semantic_content: SemanticContent,
    pub style: CellStyle,
}

/// One owned terminal grid row.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Row {
    pub cells: Vec<Cell>,
}

impl Row {
    /// Concatenates graphemes in this row, omitting empty spacer cells.
    #[must_use]
    pub fn text(&self) -> String {
        self.cells.iter().map(|cell| cell.text.as_str()).collect()
    }
}

/// Cursor location relative to the visible viewport.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Cursor {
    pub column: u16,
    pub row: u16,
    pub at_wide_tail: bool,
}

/// Terminal-wide colors needed to resolve cells without explicit colors.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TerminalColors {
    pub foreground: RgbColor,
    pub background: RgbColor,
    pub cursor: Option<RgbColor>,
}

/// Owned state suitable for transfer to application and rendering code.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Snapshot {
    pub size: GridSize,
    pub screen: ScreenKind,
    pub cursor: Option<Cursor>,
    pub colors: TerminalColors,
    pub rows: Vec<Row>,
}

/// Errors at the local terminal-core boundary.
#[derive(Debug)]
pub enum TerminalError {
    InvalidGridSize { columns: u16, rows: u16 },
    OutOfMemory,
    InvalidValue,
    OutOfSpace { required: usize },
}

impl fmt::Display for TerminalError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidGridSize { columns, rows } => {
                write!(
                    formatter,
                    "terminal grid must be non-empty, got {columns}x{rows}"
                )
            }
            Self::OutOfMemory => write!(formatter, "terminal core ran out of memory"),
            Self::InvalidValue => write!(formatter, "terminal core received an invalid value"),
            Self::OutOfSpace { required } => {
                write!(formatter, "terminal core requires a {required}-byte buffer")
            }
        }
    }
}

impl Error for TerminalError {}

impl From<libghostty_vt::Error> for TerminalError {
    fn from(error: libghostty_vt::Error) -> Self {
        match error {
            libghostty_vt::Error::OutOfMemory => Self::OutOfMemory,
            libghostty_vt::Error::InvalidValue => Self::InvalidValue,
            libghostty_vt::Error::OutOfSpace { required } => Self::OutOfSpace { required },
        }
    }
}

/// Single-thread-owned terminal emulator state.
#[derive(Debug)]
pub struct TerminalCore {
    terminal: Terminal<'static, 'static>,
    render_state: RenderState<'static>,
    row_iterator: RowIterator<'static>,
    cell_iterator: CellIterator<'static>,
}

impl TerminalCore {
    /// Creates a terminal with an application-owned scrollback limit.
    pub fn new(size: GridSize) -> Result<Self, TerminalError> {
        Ok(Self {
            terminal: Terminal::new(TerminalOptions {
                cols: size.columns,
                rows: size.rows,
                max_scrollback: DEFAULT_SCROLLBACK_ROWS,
            })?,
            render_state: RenderState::new()?,
            row_iterator: RowIterator::new()?,
            cell_iterator: CellIterator::new()?,
        })
    }

    /// Feeds raw PTY output through Ghostty's VT parser.
    pub fn write(&mut self, bytes: &[u8]) {
        self.terminal.vt_write(bytes);
    }

    /// Resizes the grid and allows Ghostty to perform terminal reflow.
    pub fn resize(&mut self, size: GridSize) -> Result<(), TerminalError> {
        self.terminal
            .resize(size.columns, size.rows, 0, 0)
            .map_err(Into::into)
    }

    /// Captures an owned frame without exposing Ghostty objects to callers.
    pub fn snapshot(&mut self) -> Result<Snapshot, TerminalError> {
        let screen = screen_kind(self.terminal.active_screen()?);
        let snapshot = self.render_state.update(&self.terminal)?;
        let size = GridSize {
            columns: snapshot.cols()?,
            rows: snapshot.rows()?,
        };
        let cursor = snapshot.cursor_viewport()?.map(cursor_from_viewport);
        let ghostty_colors = snapshot.colors()?;
        let colors = TerminalColors {
            foreground: rgb_color(ghostty_colors.foreground),
            background: rgb_color(ghostty_colors.background),
            cursor: ghostty_colors.cursor.map(rgb_color),
        };

        let mut rows = Vec::with_capacity(usize::from(size.rows));
        let mut row_iteration = self.row_iterator.update(&snapshot)?;
        while let Some(row) = row_iteration.next() {
            let mut cell_iteration = self.cell_iterator.update(row)?;
            let mut cells = Vec::with_capacity(usize::from(size.columns));
            while let Some(cell) = cell_iteration.next() {
                cells.push(capture_cell(cell)?);
            }
            rows.push(Row { cells });
        }

        Ok(Snapshot {
            size,
            screen,
            cursor,
            colors,
            rows,
        })
    }
}

fn capture_cell(cell: &CellIteration<'_, '_>) -> Result<Cell, TerminalError> {
    let raw_cell = cell.raw_cell()?;
    let style = cell.style()?;
    let mut text = String::new();
    cell.graphemes_utf8(&mut text)?;

    Ok(Cell {
        text,
        width: cell_width(raw_cell.wide()?),
        semantic_content: semantic_content(raw_cell.semantic_content()?),
        style: CellStyle {
            foreground: cell.fg_color()?.map(rgb_color),
            background: cell.bg_color()?.map(rgb_color),
            bold: style.bold,
            italic: style.italic,
            faint: style.faint,
            blink: style.blink,
            inverse: style.inverse,
            invisible: style.invisible,
            strikethrough: style.strikethrough,
            overline: style.overline,
            underline: underline(style.underline),
        },
    })
}

fn screen_kind(screen: Screen) -> ScreenKind {
    match screen {
        Screen::Primary => ScreenKind::Primary,
        Screen::Alternate => ScreenKind::Alternate,
    }
}

fn cursor_from_viewport(cursor: CursorViewport) -> Cursor {
    Cursor {
        column: cursor.x,
        row: cursor.y,
        at_wide_tail: cursor.at_wide_tail,
    }
}

fn rgb_color(color: GhosttyRgbColor) -> RgbColor {
    RgbColor {
        red: color.r,
        green: color.g,
        blue: color.b,
    }
}

fn cell_width(width: CellWide) -> CellWidth {
    match width {
        CellWide::Narrow => CellWidth::Narrow,
        CellWide::Wide => CellWidth::Wide,
        CellWide::SpacerTail => CellWidth::SpacerTail,
        CellWide::SpacerHead => CellWidth::SpacerHead,
    }
}

fn semantic_content(content: CellSemanticContent) -> SemanticContent {
    match content {
        CellSemanticContent::Output => SemanticContent::Output,
        CellSemanticContent::Input => SemanticContent::Input,
        CellSemanticContent::Prompt => SemanticContent::Prompt,
    }
}

fn underline(value: GhosttyUnderline) -> Underline {
    match value {
        GhosttyUnderline::None => Underline::None,
        GhosttyUnderline::Single => Underline::Single,
        GhosttyUnderline::Double => Underline::Double,
        GhosttyUnderline::Curly => Underline::Curly,
        GhosttyUnderline::Dotted => Underline::Dotted,
        GhosttyUnderline::Dashed => Underline::Dashed,
        _ => Underline::Unknown,
    }
}
