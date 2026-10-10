use std::collections::BTreeSet;

use crate::model::{CommandId, CommandState, CopyScope, Session};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CommandAction {
    Copy(CopyScope),
    Rerun,
    ToggleExpansion,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CommandSection {
    Input,
    Output,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum UiFocus {
    #[default]
    Composer,
    History(CommandId),
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum InputMode {
    #[default]
    Keyboard,
    Pointer,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct UiState {
    expanded: BTreeSet<CommandId>,
    pub focus: UiFocus,
    pub input_mode: InputMode,
    pub hovered_command: Option<CommandId>,
    pub hovered_section: Option<(CommandId, CommandSection)>,
    pub hovered_action: Option<(CommandId, CommandAction)>,
    pub history_scroll: f32,
}

impl UiState {
    #[must_use]
    pub fn for_session(session: &Session) -> Self {
        let expanded = session
            .commands
            .iter()
            .filter(|command| matches!(command.state, CommandState::Running))
            .map(|command| command.id)
            .collect();

        Self {
            expanded,
            ..Self::default()
        }
    }

    #[must_use]
    pub fn is_expanded(&self, id: CommandId) -> bool {
        self.expanded.contains(&id)
    }

    #[must_use]
    pub fn with_toggled_expansion(&self, id: CommandId) -> Self {
        let mut expanded = self.expanded.clone();
        if !expanded.remove(&id) {
            expanded.insert(id);
        }

        Self {
            expanded,
            ..self.clone()
        }
    }

    #[must_use]
    pub fn with_hover(
        &self,
        hovered_command: Option<CommandId>,
        hovered_section: Option<(CommandId, CommandSection)>,
        hovered_action: Option<(CommandId, CommandAction)>,
    ) -> Self {
        Self {
            input_mode: InputMode::Pointer,
            hovered_command,
            hovered_section,
            hovered_action,
            ..self.clone()
        }
    }

    #[must_use]
    pub fn with_keyboard_focus(&self, focus: UiFocus) -> Self {
        Self {
            focus,
            input_mode: InputMode::Keyboard,
            ..self.clone()
        }
    }

    #[must_use]
    pub fn focused_command(&self) -> Option<CommandId> {
        match self.focus {
            UiFocus::Composer => None,
            UiFocus::History(id) => Some(id),
        }
    }

    #[must_use]
    pub fn shows_keyboard_focus(&self, id: CommandId) -> bool {
        self.input_mode == InputMode::Keyboard && self.focus == UiFocus::History(id)
    }

    #[must_use]
    pub fn shows_command_actions(&self, id: CommandId) -> bool {
        match self.input_mode {
            InputMode::Keyboard => self.focus == UiFocus::History(id),
            InputMode::Pointer => {
                self.hovered_command == Some(id) && self.hovered_section.is_none()
            }
        }
    }

    #[must_use]
    pub fn shows_section_action(&self, id: CommandId, section: CommandSection) -> bool {
        self.input_mode == InputMode::Pointer && self.hovered_section == Some((id, section))
    }

    #[must_use]
    pub fn is_pointer_hovered(&self, id: CommandId) -> bool {
        self.input_mode == InputMode::Pointer && self.hovered_command == Some(id)
    }

    #[must_use]
    pub fn with_scroll_delta(&self, delta: f32, limit: f32) -> Self {
        Self {
            history_scroll: (self.history_scroll + delta).clamp(0.0, limit),
            ..self.clone()
        }
    }

    #[must_use]
    pub fn with_history_scroll(&self, history_scroll: f32, limit: f32) -> Self {
        Self {
            history_scroll: history_scroll.clamp(0.0, limit),
            ..self.clone()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn running_commands_start_expanded() {
        let session = Session::prototype();
        let ui = UiState::for_session(&session);

        assert!(ui.is_expanded(CommandId(3)));
        assert!(!ui.is_expanded(CommandId(2)));
    }

    #[test]
    fn commands_expand_independently_and_toggle_closed() {
        let session = Session::prototype();
        let initial = UiState::for_session(&session);
        let two_expanded = initial.with_toggled_expansion(CommandId(1));
        let collapsed_again = two_expanded.with_toggled_expansion(CommandId(1));

        assert!(two_expanded.is_expanded(CommandId(1)));
        assert!(two_expanded.is_expanded(CommandId(3)));
        assert!(!collapsed_again.is_expanded(CommandId(1)));
        assert!(collapsed_again.is_expanded(CommandId(3)));
    }
}
