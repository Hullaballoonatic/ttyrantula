use crate::model::{Command, CommandId, CommandState, CopyScope, Metadata};

use super::{
    geometry::{
        Quad, Rect, Rgba, TextWeight, border_quads, quad, rect, rgba8, right_aligned_text, text,
    },
    scene::{
        ACCENT_TEXT, ACTION_BACKGROUND, ACTION_HOVERED, BORDER, BORDER_EXPANDED, BORDER_FOCUSED,
        CARD, CARD_HOVERED, CommandActionTarget, CommandSectionTarget, CommandTarget, FAILURE,
        INPUT, Metrics, RUNNING, SUCCESS, Scene, TEXT, TEXT_MUTED, TEXT_SUBTLE,
    },
    state::{CommandAction, CommandSection, InputMode, UiState},
};

pub(super) fn append_command(
    scene: &mut Scene,
    command: &Command,
    bounds: Rect,
    is_expanded: bool,
    ui: &UiState,
    metrics: Metrics,
) {
    let border_color = if ui.shows_keyboard_focus(command.id) {
        BORDER_FOCUSED
    } else if is_expanded {
        BORDER_EXPANDED
    } else {
        BORDER
    };
    let card_color = if ui.is_pointer_hovered(command.id) {
        CARD_HOVERED
    } else {
        CARD
    };

    scene.quads.push(Quad {
        bounds,
        color: card_color,
    });
    scene
        .quads
        .extend(border_quads(bounds, metrics.dip(1.0), border_color));
    scene.command_targets.push(CommandTarget {
        id: command.id,
        bounds,
    });

    if is_expanded {
        append_expanded_command(scene, command, bounds, ui, metrics);
    } else {
        append_summary_command(scene, command, bounds, metrics);
    }

    append_action_set(scene, command.id, bounds, is_expanded, ui, metrics);
}

fn append_summary_command(scene: &mut Scene, command: &Command, bounds: Rect, metrics: Metrics) {
    let accent = status_color(&command.state);
    scene.quads.push(quad(
        bounds.x,
        bounds.y,
        metrics.dip(3.0),
        bounds.height,
        accent,
    ));
    scene.text.push(text(
        &command.input,
        rect(
            bounds.x + metrics.dip(18.0),
            bounds.y + metrics.dip(18.0),
            (bounds.width - metrics.dip(188.0)).max(metrics.dip(80.0)),
            metrics.dip(24.0),
        ),
        TEXT,
        metrics.dip(14.0),
        metrics.dip(21.0),
        TextWeight::Normal,
    ));
    scene.text.push(right_aligned_text(
        status_label(command),
        rect(
            bounds.x + (bounds.width - metrics.dip(138.0)).max(metrics.dip(18.0)),
            bounds.y + metrics.dip(20.0),
            metrics.dip(120.0),
            metrics.dip(18.0),
        ),
        rgba8(accent),
        metrics.dip(10.5),
        metrics.dip(16.0),
        TextWeight::Semibold,
    ));
}

fn append_expanded_command(
    scene: &mut Scene,
    command: &Command,
    bounds: Rect,
    ui: &UiState,
    metrics: Metrics,
) {
    let input_top = bounds.y + metrics.dip(44.0);
    let input_height = metrics.dip(51.0);
    let content_left = bounds.x + metrics.dip(18.0);
    let content_width = bounds.width - metrics.dip(36.0);
    let accent = status_color(&command.state);
    let input_bounds = rect(
        bounds.x + metrics.dip(1.0),
        input_top,
        bounds.width - metrics.dip(2.0),
        input_height,
    );
    let output_bounds = rect(
        bounds.x + metrics.dip(1.0),
        input_top + input_height,
        bounds.width - metrics.dip(2.0),
        bounds.y + bounds.height - input_top - input_height,
    );

    scene.quads.push(quad(
        bounds.x,
        bounds.y,
        metrics.dip(3.0),
        bounds.height,
        accent,
    ));
    scene.quads.push(Quad {
        bounds: input_bounds,
        color: INPUT,
    });
    scene.section_targets.extend([
        CommandSectionTarget {
            id: command.id,
            section: CommandSection::Input,
            bounds: input_bounds,
        },
        CommandSectionTarget {
            id: command.id,
            section: CommandSection::Output,
            bounds: output_bounds,
        },
    ]);
    scene.text.push(text(
        metadata_label(&command.metadata),
        rect(
            content_left,
            bounds.y + metrics.dip(14.0),
            (content_width - metrics.dip(120.0)).max(metrics.dip(60.0)),
            metrics.dip(18.0),
        ),
        TEXT_MUTED,
        metrics.dip(11.5),
        metrics.dip(17.0),
        TextWeight::Normal,
    ));
    scene.text.push(right_aligned_text(
        status_label(command),
        rect(
            bounds.x + (bounds.width - metrics.dip(138.0)).max(metrics.dip(18.0)),
            bounds.y + metrics.dip(14.0),
            metrics.dip(120.0),
            metrics.dip(18.0),
        ),
        rgba8(accent),
        metrics.dip(10.5),
        metrics.dip(16.0),
        TextWeight::Semibold,
    ));
    scene.text.push(text(
        "❯",
        rect(
            content_left,
            input_top + metrics.dip(14.0),
            metrics.dip(20.0),
            metrics.dip(25.0),
        ),
        ACCENT_TEXT,
        metrics.dip(15.0),
        metrics.dip(22.0),
        TextWeight::Semibold,
    ));
    scene.text.push(text(
        &command.input,
        rect(
            content_left + metrics.dip(27.0),
            input_top + metrics.dip(14.0),
            content_width - metrics.dip(27.0),
            metrics.dip(25.0),
        ),
        TEXT,
        metrics.dip(14.0),
        metrics.dip(22.0),
        TextWeight::Normal,
    ));
    scene.text.push(text(
        &command.output,
        rect(
            content_left,
            input_top + input_height + metrics.dip(14.0),
            content_width,
            bounds.height - input_height - metrics.dip(70.0),
        ),
        TEXT_MUTED,
        metrics.dip(12.5),
        metrics.dip(19.0),
        TextWeight::Normal,
    ));
    append_section_copy_action(
        scene,
        command.id,
        CommandSection::Input,
        input_bounds,
        ui,
        metrics,
    );
    append_section_copy_action(
        scene,
        command.id,
        CommandSection::Output,
        output_bounds,
        ui,
        metrics,
    );
}

fn append_section_copy_action(
    scene: &mut Scene,
    command_id: CommandId,
    section: CommandSection,
    section_bounds: Rect,
    ui: &UiState,
    metrics: Metrics,
) {
    let button_size = metrics.dip(32.0);
    let bounds = rect(
        section_bounds.x + section_bounds.width - button_size - metrics.dip(7.0),
        section_bounds.y + metrics.dip(6.0),
        button_size,
        button_size,
    );
    let scope = match section {
        CommandSection::Input => CopyScope::Input,
        CommandSection::Output => CopyScope::Output,
    };
    let action = CommandAction::Copy(scope);

    scene.action_targets.push(CommandActionTarget {
        id: command_id,
        action,
        bounds,
        enabled: true,
    });
    if !ui.shows_section_action(command_id, section) {
        return;
    }

    scene.quads.push(Quad {
        bounds,
        color: if ui.hovered_action == Some((command_id, action)) {
            ACTION_HOVERED
        } else {
            ACTION_BACKGROUND
        },
    });
    scene.text.push(text(
        "⧉",
        rect(
            bounds.x + metrics.dip(9.0),
            bounds.y + metrics.dip(6.0),
            metrics.dip(17.0),
            metrics.dip(20.0),
        ),
        TEXT_MUTED,
        metrics.dip(15.0),
        metrics.dip(20.0),
        TextWeight::Semibold,
    ));
}

fn append_action_set(
    scene: &mut Scene,
    command_id: CommandId,
    command_bounds: Rect,
    is_expanded: bool,
    ui: &UiState,
    metrics: Metrics,
) {
    let button_size = metrics.dip(32.0);
    let gap = metrics.dip(2.0);
    let actions = [
        (CommandAction::Copy(CopyScope::All), "⧉", true),
        (CommandAction::Rerun, "↻", false),
        (
            CommandAction::ToggleExpansion,
            if is_expanded { "⌄" } else { "›" },
            true,
        ),
    ];
    let set_width = button_size * actions.len() as f32 + gap * 2.0;
    let top = command_bounds.y
        + if is_expanded {
            metrics.dip(6.0)
        } else {
            (command_bounds.height - button_size) / 2.0
        };
    let left = command_bounds.x + command_bounds.width - set_width - metrics.dip(7.0);

    actions
        .into_iter()
        .enumerate()
        .for_each(|(index, (action, icon, enabled))| {
            let bounds = rect(
                left + index as f32 * (button_size + gap),
                top,
                button_size,
                button_size,
            );
            scene.action_targets.push(CommandActionTarget {
                id: command_id,
                action,
                bounds,
                enabled,
            });

            if !ui.shows_command_actions(command_id) {
                return;
            }

            let is_hovered = ui.input_mode == InputMode::Pointer
                && ui.hovered_action == Some((command_id, action));
            scene.quads.push(Quad {
                bounds,
                color: if is_hovered && enabled {
                    ACTION_HOVERED
                } else {
                    ACTION_BACKGROUND
                },
            });
            scene.text.push(text(
                icon,
                rect(
                    bounds.x + metrics.dip(9.0),
                    bounds.y + metrics.dip(6.0),
                    metrics.dip(17.0),
                    metrics.dip(20.0),
                ),
                if enabled { TEXT_MUTED } else { TEXT_SUBTLE },
                metrics.dip(15.0),
                metrics.dip(20.0),
                TextWeight::Semibold,
            ));
        });
}

fn metadata_label(metadata: &Metadata) -> String {
    metadata
        .branch
        .as_deref()
        .map(|branch| format!("{}  ·  {branch}", metadata.working_directory))
        .unwrap_or_else(|| metadata.working_directory.clone())
}

fn status_label(command: &Command) -> String {
    match (&command.state, command.duration) {
        (CommandState::Running, _) => "RUNNING".into(),
        (CommandState::Completed { exit_status: 0 }, Some(duration)) => {
            format_duration(duration.as_millis())
        }
        (CommandState::Completed { exit_status: 0 }, None) => "DONE".into(),
        (CommandState::Completed { exit_status }, _) => format!("EXIT {exit_status}"),
    }
}

pub(super) fn format_duration(milliseconds: u128) -> String {
    if milliseconds < 1_000 {
        format!("{milliseconds} ms")
    } else {
        format!("{:.1} s", milliseconds as f64 / 1_000.0)
    }
}

fn status_color(state: &CommandState) -> Rgba {
    match state {
        CommandState::Running => RUNNING,
        CommandState::Completed { exit_status: 0 } => SUCCESS,
        CommandState::Completed { .. } => FAILURE,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn duration_labels_choose_readable_units() {
        assert_eq!(format_duration(84), "84 ms");
        assert_eq!(format_duration(2_841), "2.8 s");
    }
}
