use ttyrantula::terminal::{
    CellWidth, GridSize, RgbColor, ScreenKind, SemanticContent, TerminalCore,
};

#[test]
fn composer_separates_hint_suffix_from_actual_input() {
    let mut terminal = TerminalCore::new(GridSize::new(80, 24).unwrap()).unwrap();
    terminal.write(b"\x1b]133;A\x07nu> \x1b]133;B\x07l\x1b[2;38;2;128;128;128ms\x1b[0m\x1b[1D");
    let snapshot = terminal.snapshot().unwrap();
    assert_eq!(snapshot.composer_input().as_deref(), Some("l"));
    assert_eq!(snapshot.composer_hint(), "s");
}

fn terminal(columns: u16, rows: u16) -> TerminalCore {
    TerminalCore::new(GridSize::new(columns, rows).unwrap()).unwrap()
}

fn row_text(core: &mut TerminalCore, row: usize) -> String {
    core.snapshot().unwrap().rows[row].text()
}

#[test]
fn records_text_color_cursor_movement_and_unicode() {
    const STREAM: &[u8] = b"abc\x1b[2D\x1b[38;2;12;34;56m!\x1b[0m \xe7\x95\x8c e\xcc\x81";

    let mut core = terminal(16, 3);
    core.write(STREAM);
    let snapshot = core.snapshot().unwrap();

    assert_eq!(snapshot.rows[0].text(), "a! 界 e\u{301}");
    assert_eq!(snapshot.cursor.unwrap().column, 7);
    assert_eq!(
        snapshot.rows[0].cells[1].style.foreground,
        Some(RgbColor {
            red: 12,
            green: 34,
            blue: 56,
        })
    );
    assert_eq!(snapshot.rows[0].cells[3].width, CellWidth::Wide);
    assert_eq!(snapshot.rows[0].cells[4].width, CellWidth::SpacerTail);
}

#[test]
fn resize_reflows_primary_screen_content() {
    let mut core = terminal(8, 3);
    core.write(b"abcdefghijk");
    core.resize(GridSize::new(5, 4).unwrap()).unwrap();
    let snapshot = core.snapshot().unwrap();

    assert_eq!(snapshot.size, GridSize::new(5, 4).unwrap());
    assert_eq!(snapshot.rows[0].text(), "abcde");
    assert_eq!(snapshot.rows[1].text(), "fghij");
    assert_eq!(snapshot.rows[2].text(), "k");
}

#[test]
fn alternate_screen_is_distinct_and_primary_screen_is_restored() {
    let mut core = terminal(12, 3);
    core.write(b"primary");
    core.write(b"\x1b[?1049halt");

    let alternate = core.snapshot().unwrap();
    assert_eq!(alternate.screen, ScreenKind::Alternate);
    assert_eq!(alternate.rows[0].text(), "alt");

    core.write(b"\x1b[?1049l");
    let primary = core.snapshot().unwrap();
    assert_eq!(primary.screen, ScreenKind::Primary);
    assert_eq!(primary.rows[0].text(), "primary");
}

#[test]
fn osc_133_marks_prompt_input_and_output_cells() {
    const STREAM: &[u8] =
        b"\x1b]133;A\x1b\\$ \x1b]133;B\x1b\\echo ok\x1b]133;C\x1b\\\r\nok\x1b]133;D;0\x1b\\";

    let mut core = terminal(20, 3);
    core.write(STREAM);
    let snapshot = core.snapshot().unwrap();
    let first_row = &snapshot.rows[0].cells;
    let second_row = &snapshot.rows[1].cells;

    assert!(
        first_row[0..2]
            .iter()
            .all(|cell| cell.semantic_content == SemanticContent::Prompt)
    );
    assert!(
        first_row[2..9]
            .iter()
            .all(|cell| cell.semantic_content == SemanticContent::Input)
    );
    assert!(
        second_row[0..2]
            .iter()
            .all(|cell| cell.semantic_content == SemanticContent::Output)
    );
}

#[test]
fn rejects_an_empty_grid() {
    assert!(GridSize::new(0, 24).is_err());
    assert!(GridSize::new(80, 0).is_err());
}

#[test]
fn plain_text_remains_available_without_semantic_sequences() {
    let mut core = terminal(10, 2);
    core.write(b"ordinary");

    assert_eq!(row_text(&mut core, 0), "ordinary");
}
#[test]
fn composer_projects_shell_edited_input_without_prompt() {
    let mut terminal = TerminalCore::new(GridSize::new(80, 24).unwrap()).unwrap();
    terminal.write(b"\x1b]133;A\x07nu> \x1b]133;B\x07print hello");
    assert_eq!(
        terminal.snapshot().unwrap().composer_input().as_deref(),
        Some("print hello")
    );
    terminal.write(b"\x08 \x08");
    assert_eq!(
        terminal.snapshot().unwrap().composer_input().as_deref(),
        Some("print hell")
    );
}
