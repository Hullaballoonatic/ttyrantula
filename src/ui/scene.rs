use crate::model::{CommandId, Session};

use super::{
    command::append_command,
    geometry::{Point, Quad, Rect, Rgba, Text, TextWeight, quad, rect, text},
    state::{CommandAction, CommandSection, UiFocus, UiState},
};

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CommandTarget {
    pub id: CommandId,
    pub bounds: Rect,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CommandActionTarget {
    pub id: CommandId,
    pub action: CommandAction,
    pub bounds: Rect,
    pub enabled: bool,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CommandSectionTarget {
    pub id: CommandId,
    pub section: CommandSection,
    pub bounds: Rect,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Scene {
    pub quads: Vec<Quad>,
    pub text: Vec<Text>,
    pub command_targets: Vec<CommandTarget>,
    pub command_layout: Vec<CommandTarget>,
    pub action_targets: Vec<CommandActionTarget>,
    pub section_targets: Vec<CommandSectionTarget>,
    pub history_bounds: Rect,
    pub history_scroll_limit: f32,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct Metrics {
    scale: f32,
    width: f32,
    height: f32,
}

impl Metrics {
    pub(super) fn dip(self, value: f32) -> f32 {
        value * self.scale
    }
}

pub(super) const BACKGROUND: Rgba = [0.043, 0.047, 0.047, 1.0];
pub(super) const HEADER: Rgba = [0.059, 0.063, 0.062, 1.0];
pub(super) const COMPOSER: Rgba = [0.075, 0.080, 0.078, 1.0];
pub(super) const CARD: Rgba = [0.071, 0.076, 0.074, 1.0];
pub(super) const CARD_HOVERED: Rgba = [0.090, 0.096, 0.093, 1.0];
pub(super) const INPUT: Rgba = [0.094, 0.101, 0.097, 1.0];
pub(super) const BORDER: Rgba = [0.19, 0.21, 0.20, 1.0];
pub(super) const BORDER_EXPANDED: Rgba = [0.35, 0.78, 0.59, 1.0];
pub(super) const BORDER_FOCUSED: Rgba = [0.38, 0.63, 0.91, 1.0];
pub(super) const ACTION_BACKGROUND: Rgba = [0.12, 0.13, 0.13, 1.0];
pub(super) const ACTION_HOVERED: Rgba = [0.19, 0.21, 0.20, 1.0];
pub(super) const SUCCESS: Rgba = [0.31, 0.73, 0.52, 1.0];
pub(super) const FAILURE: Rgba = [0.92, 0.40, 0.35, 1.0];
pub(super) const RUNNING: Rgba = [0.96, 0.72, 0.30, 1.0];
pub(super) const TEXT: [u8; 4] = [229, 234, 230, 255];
pub(super) const TEXT_MUTED: [u8; 4] = [143, 153, 147, 255];
pub(super) const TEXT_SUBTLE: [u8; 4] = [105, 115, 109, 255];
pub(super) const ACCENT_TEXT: [u8; 4] = [125, 218, 166, 255];

#[must_use]
pub fn build_scene(
    session: &Session,
    ui: &UiState,
    width: u32,
    height: u32,
    scale_factor: f32,
) -> Scene {
    let metrics = Metrics {
        scale: scale_factor.max(0.5),
        width: width as f32,
        height: height as f32,
    };
    let header_height = metrics.dip(52.0);
    let composer_height = metrics.dip(122.0).min(metrics.height * 0.34);
    let composer_top = metrics.height - composer_height;

    let mut scene = Scene {
        quads: vec![
            quad(0.0, 0.0, metrics.width, metrics.height, BACKGROUND),
            quad(0.0, 0.0, metrics.width, header_height, HEADER),
            quad(0.0, composer_top, metrics.width, composer_height, COMPOSER),
            quad(
                0.0,
                composer_top,
                metrics.width,
                metrics.dip(1.0),
                if ui.focus == UiFocus::Composer {
                    BORDER_EXPANDED
                } else {
                    BORDER
                },
            ),
        ],
        ..Scene::default()
    };

    scene.text.extend(header_text(metrics));
    scene
        .text
        .extend(composer_text(session, ui, metrics, composer_top));

    let horizontal_margin = metrics.dip(22.0).min(metrics.width * 0.06);
    let gap = metrics.dip(10.0);
    let history_top = header_height + metrics.dip(18.0);
    let history_bottom = composer_top - metrics.dip(16.0);
    let card_width = (metrics.width - horizontal_margin * 2.0).max(0.0);
    let history_bounds = rect(
        0.0,
        history_top,
        metrics.width,
        history_bottom - history_top,
    );
    let expanded_height = metrics.dip(218.0);
    let summary_height = metrics.dip(58.0);
    let content_height = session
        .commands
        .iter()
        .map(|command| {
            if ui.is_expanded(command.id) {
                expanded_height
            } else {
                summary_height
            }
        })
        .sum::<f32>()
        + gap * session.commands.len().saturating_sub(1) as f32;
    let history_scroll_limit = (content_height - history_bounds.height).max(0.0);
    let history_scroll = ui.history_scroll.clamp(0.0, history_scroll_limit);

    let cards = session
        .commands
        .iter()
        .rev()
        .scan(history_bottom + history_scroll, |bottom, command| {
            let is_expanded = ui.is_expanded(command.id);
            let height = if is_expanded {
                expanded_height
            } else {
                summary_height
            };
            let bounds = Rect {
                x: horizontal_margin,
                y: *bottom - height,
                width: card_width,
                height,
            };
            *bottom = bounds.y - gap;
            Some((command, bounds, is_expanded))
        })
        .collect::<Vec<_>>();

    scene.command_layout = cards
        .iter()
        .map(|(command, bounds, _)| CommandTarget {
            id: command.id,
            bounds: *bounds,
        })
        .collect();

    let mut history_scene = Scene::default();
    cards
        .into_iter()
        .filter(|(_, bounds, _)| bounds.intersection(history_bounds).is_some())
        .for_each(|(command, bounds, is_expanded)| {
            append_command(
                &mut history_scene,
                command,
                bounds,
                is_expanded,
                ui,
                metrics,
            );
        });
    append_clipped_history(&mut scene, history_scene, history_bounds);
    scene.history_bounds = history_bounds;
    scene.history_scroll_limit = history_scroll_limit;

    scene
}

#[must_use]
pub fn hit_test_command(scene: &Scene, point: Point) -> Option<CommandId> {
    scene
        .command_targets
        .iter()
        .find(|target| target.bounds.contains(point))
        .map(|target| target.id)
}

#[must_use]
pub fn hit_test_action(scene: &Scene, point: Point) -> Option<CommandActionTarget> {
    scene
        .action_targets
        .iter()
        .find(|target| target.bounds.contains(point))
        .copied()
}

#[must_use]
pub fn hit_test_section(scene: &Scene, point: Point) -> Option<(CommandId, CommandSection)> {
    scene
        .section_targets
        .iter()
        .find(|target| target.bounds.contains(point))
        .map(|target| (target.id, target.section))
}

#[must_use]
pub fn scroll_to_reveal_command(scene: &Scene, current_scroll: f32, id: CommandId) -> f32 {
    let Some(target) = scene.command_layout.iter().find(|target| target.id == id) else {
        return current_scroll;
    };
    let target_bottom = target.bounds.y + target.bounds.height;
    let history_bottom = scene.history_bounds.y + scene.history_bounds.height;
    let desired_scroll = if target.bounds.y < scene.history_bounds.y {
        current_scroll + scene.history_bounds.y - target.bounds.y
    } else if target_bottom > history_bottom {
        current_scroll - (target_bottom - history_bottom)
    } else {
        current_scroll
    };

    desired_scroll.clamp(0.0, scene.history_scroll_limit)
}

fn header_text(metrics: Metrics) -> impl Iterator<Item = Text> {
    [
        text(
            "ttyrantula",
            rect(
                metrics.dip(22.0),
                metrics.dip(14.0),
                metrics.dip(220.0),
                metrics.dip(28.0),
            ),
            TEXT,
            metrics.dip(18.0),
            metrics.dip(24.0),
            TextWeight::Semibold,
        ),
        text(
            "MOCK SESSION",
            rect(
                (metrics.width - metrics.dip(154.0)).max(metrics.dip(160.0)),
                metrics.dip(17.0),
                metrics.dip(132.0),
                metrics.dip(20.0),
            ),
            TEXT_SUBTLE,
            metrics.dip(11.0),
            metrics.dip(16.0),
            TextWeight::Semibold,
        ),
    ]
    .into_iter()
}

fn composer_text(
    session: &Session,
    ui: &UiState,
    metrics: Metrics,
    top: f32,
) -> impl Iterator<Item = Text> {
    let branch = session
        .composer
        .branch
        .as_deref()
        .map(|branch| format!("  {branch}"))
        .unwrap_or_default();

    [
        text(
            format!("{}{}", session.composer.working_directory, branch),
            rect(
                metrics.dip(24.0),
                top + metrics.dip(19.0),
                metrics.width - metrics.dip(48.0),
                metrics.dip(20.0),
            ),
            TEXT_MUTED,
            metrics.dip(12.5),
            metrics.dip(18.0),
            TextWeight::Normal,
        ),
        text(
            "❯",
            rect(
                metrics.dip(24.0),
                top + metrics.dip(57.0),
                metrics.dip(24.0),
                metrics.dip(30.0),
            ),
            ACCENT_TEXT,
            metrics.dip(18.0),
            metrics.dip(26.0),
            TextWeight::Semibold,
        ),
        text(
            if ui.focus == UiFocus::Composer {
                format!("{}▏", session.composer.input)
            } else {
                session.composer.input.clone()
            },
            rect(
                metrics.dip(53.0),
                top + metrics.dip(57.0),
                metrics.width - metrics.dip(80.0),
                metrics.dip(30.0),
            ),
            TEXT,
            metrics.dip(16.0),
            metrics.dip(25.0),
            TextWeight::Normal,
        ),
    ]
    .into_iter()
}

fn append_clipped_history(scene: &mut Scene, history: Scene, clip: Rect) {
    scene
        .quads
        .extend(history.quads.into_iter().filter_map(|quad| {
            quad.bounds.intersection(clip).map(|bounds| Quad {
                bounds,
                color: quad.color,
            })
        }));
    scene
        .text
        .extend(history.text.into_iter().filter_map(|text| {
            text.bounds.intersection(clip).map(|clip_bounds| Text {
                clip_bounds,
                ..text
            })
        }));
    scene
        .command_targets
        .extend(clipped_targets(history.command_targets, clip));
    scene
        .action_targets
        .extend(clipped_action_targets(history.action_targets, clip));
    scene
        .section_targets
        .extend(clipped_section_targets(history.section_targets, clip));
}

fn clipped_targets(targets: Vec<CommandTarget>, clip: Rect) -> impl Iterator<Item = CommandTarget> {
    targets.into_iter().filter_map(move |target| {
        target
            .bounds
            .intersection(clip)
            .map(|bounds| CommandTarget {
                id: target.id,
                bounds,
            })
    })
}

fn clipped_action_targets(
    targets: Vec<CommandActionTarget>,
    clip: Rect,
) -> impl Iterator<Item = CommandActionTarget> {
    targets.into_iter().filter_map(move |target| {
        target
            .bounds
            .intersection(clip)
            .map(|bounds| CommandActionTarget { bounds, ..target })
    })
}

fn clipped_section_targets(
    targets: Vec<CommandSectionTarget>,
    clip: Rect,
) -> impl Iterator<Item = CommandSectionTarget> {
    targets.into_iter().filter_map(move |target| {
        target
            .bounds
            .intersection(clip)
            .map(|bounds| CommandSectionTarget { bounds, ..target })
    })
}

#[cfg(test)]
mod tests {
    use super::super::geometry::TextAlign;
    use super::*;
    use crate::model::CopyScope;

    #[test]
    fn history_stays_above_composer() {
        let session = Session::prototype();
        let scene = build_scene(&session, &UiState::for_session(&session), 1_200, 800, 1.0);

        assert!(
            scene
                .command_targets
                .iter()
                .all(|target| target.bounds.intersection(scene.history_bounds)
                    == Some(target.bounds))
        );
    }

    #[test]
    fn expanded_command_has_expanded_height() {
        let session = Session::prototype();
        let expanded = UiState::default().with_toggled_expansion(CommandId(2));
        let scene = build_scene(&session, &expanded, 1_200, 800, 1.0);
        let expanded_height = scene
            .command_targets
            .iter()
            .find(|target| target.id == CommandId(2))
            .map(|target| target.bounds.height);

        assert_eq!(expanded_height, Some(218.0));
    }

    #[test]
    fn command_actions_have_dedicated_hit_targets() {
        let session = Session::prototype();
        let scene = build_scene(&session, &UiState::default(), 1_200, 800, 1.0);
        let target = scene
            .command_targets
            .iter()
            .find(|target| target.id == CommandId(2))
            .copied()
            .expect("collapsed prototype command is visible");
        let command_point = Point {
            x: target.bounds.x + 10.0,
            y: target.bounds.y + 10.0,
        };
        let action_targets = scene
            .action_targets
            .iter()
            .filter(|action| action.id == target.id)
            .copied()
            .collect::<Vec<_>>();
        let disclosure = action_targets
            .iter()
            .find(|action| action.action == CommandAction::ToggleExpansion)
            .copied()
            .expect("visible commands have disclosure actions");
        let action_point = Point {
            x: disclosure.bounds.x + disclosure.bounds.width / 2.0,
            y: disclosure.bounds.y + disclosure.bounds.height / 2.0,
        };

        assert_eq!(action_targets.len(), 3);
        assert_eq!(
            action_targets
                .iter()
                .filter(|action| action.enabled)
                .count(),
            2
        );
        assert_eq!(hit_test_command(&scene, command_point), Some(target.id));
        assert_eq!(hit_test_action(&scene, command_point), None);
        assert_eq!(hit_test_action(&scene, action_point), Some(disclosure));
    }

    #[test]
    fn command_action_set_is_only_drawn_for_hovered_command() {
        let session = Session::prototype();
        let hidden = build_scene(&session, &UiState::default(), 1_200, 800, 1.0);
        let command_id = hidden.command_targets[0].id;
        let hovered = build_scene(
            &session,
            &UiState::default().with_hover(Some(command_id), None, None),
            1_200,
            800,
            1.0,
        );
        let is_action_icon = |text: &Text| ["⧉", "↻", "›", "⌄"].contains(&text.content.as_str());

        assert!(!hidden.text.iter().any(is_action_icon));
        assert_eq!(
            hovered
                .text
                .iter()
                .filter(|text| is_action_icon(text))
                .count(),
            3
        );
        assert_eq!(
            hidden.text,
            hovered
                .text
                .into_iter()
                .filter(|text| !is_action_icon(text))
                .collect::<Vec<_>>()
        );
    }

    #[test]
    fn keyboard_focus_reveals_actions_until_pointer_input_resumes() {
        let session = Session::prototype();
        let command_id = CommandId(2);
        let focused = UiState::default()
            .with_hover(Some(CommandId(1)), None, None)
            .with_keyboard_focus(UiFocus::History(command_id));
        let pointer = focused.with_hover(None, None, None);
        let focused_scene = build_scene(&session, &focused, 1_200, 800, 1.0);
        let pointer_scene = build_scene(&session, &pointer, 1_200, 800, 1.0);
        let is_action_icon = |text: &Text| ["⧉", "↻", "›", "⌄"].contains(&text.content.as_str());

        assert_eq!(
            focused_scene
                .text
                .iter()
                .filter(|text| is_action_icon(text))
                .count(),
            3
        );
        assert!(!pointer_scene.text.iter().any(is_action_icon));
    }

    #[test]
    fn section_hover_replaces_the_primary_action_set() {
        let session = Session::prototype();
        let command_id = CommandId(3);
        let ui = UiState::for_session(&session).with_hover(
            Some(command_id),
            Some((command_id, CommandSection::Input)),
            None,
        );
        let scene = build_scene(&session, &ui, 1_200, 800, 1.0);
        let copy_icons = scene.text.iter().filter(|text| text.content == "⧉").count();

        assert_eq!(copy_icons, 1);
        assert!(scene.action_targets.iter().any(|target| {
            target.id == command_id
                && target.action == CommandAction::Copy(CopyScope::Input)
                && target.enabled
        }));
        assert!(!scene.text.iter().any(|text| text.content == "↻"));
    }

    #[test]
    fn expanded_command_sections_are_hit_testable() {
        let session = Session::prototype();
        let scene = build_scene(&session, &UiState::for_session(&session), 1_200, 800, 1.0);
        let input = scene
            .section_targets
            .iter()
            .find(|target| target.id == CommandId(3) && target.section == CommandSection::Input)
            .expect("running command input is visible");
        let point = Point {
            x: input.bounds.x + 10.0,
            y: input.bounds.y + 10.0,
        };

        assert_eq!(
            hit_test_section(&scene, point),
            Some((CommandId(3), CommandSection::Input))
        );
    }

    #[test]
    fn keyboard_navigation_can_reveal_an_offscreen_command() {
        let session = Session::prototype();
        let initial = UiState::for_session(&session);
        let initial_scene = build_scene(&session, &initial, 520, 420, 1.0);
        let scroll = scroll_to_reveal_command(&initial_scene, 0.0, CommandId(0));
        let revealed = initial.with_history_scroll(scroll, initial_scene.history_scroll_limit);
        let revealed_scene = build_scene(&session, &revealed, 520, 420, 1.0);

        assert!(scroll > 0.0);
        assert!(
            revealed_scene
                .command_targets
                .iter()
                .any(|target| target.id == CommandId(0))
        );
    }

    #[test]
    fn scroll_is_clamped_to_the_history_content() {
        let session = Session::prototype();
        let initial = UiState::for_session(&session);
        let scene = build_scene(&session, &initial, 520, 420, 1.0);
        let scrolled = initial.with_scroll_delta(f32::MAX, scene.history_scroll_limit);

        assert!(scene.history_scroll_limit > 0.0);
        assert_eq!(scrolled.history_scroll, scene.history_scroll_limit);
    }

    #[test]
    fn command_status_is_right_aligned_to_the_card_padding() {
        let session = Session::prototype();
        let scene = build_scene(&session, &UiState::default(), 1_200, 800, 1.0);
        let command = scene
            .command_targets
            .iter()
            .find(|target| target.id == CommandId(0))
            .expect("prototype command is visible");
        let status = scene
            .text
            .iter()
            .find(|text| text.content == "84 ms")
            .expect("prototype command has a duration");

        assert_eq!(status.align, TextAlign::Right);
        assert_eq!(
            status.bounds.x + status.bounds.width,
            command.bounds.x + command.bounds.width - 18.0
        );
    }
}
