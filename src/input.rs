use crate::{
    model::{Command, CommandId, CopyScope},
    ui::CommandAction,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum InputContext {
    Composer,
    History,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Key {
    Character(char),
    ArrowUp,
    ArrowDown,
    Space,
    Enter,
    Escape,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct Modifiers {
    pub control: bool,
    pub alt: bool,
    pub shift: bool,
    pub super_key: bool,
}

impl Modifiers {
    const NONE: Self = Self {
        control: false,
        alt: false,
        shift: false,
        super_key: false,
    };

    const CONTROL: Self = Self {
        control: true,
        ..Self::NONE
    };
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct KeyChord {
    pub key: Key,
    pub modifiers: Modifiers,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum HistoryDirection {
    Older,
    Newer,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ApplicationAction {
    FocusComposer,
    FocusHistory,
    Paste,
    MoveHistoryFocus(HistoryDirection),
    Command(CommandAction),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct KeyBinding {
    pub context: InputContext,
    pub first: KeyChord,
    pub second: Option<KeyChord>,
    pub action: ApplicationAction,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct KeySequenceState {
    pending: Option<KeyChord>,
}

impl KeySequenceState {
    #[must_use]
    pub fn is_pending(self) -> bool {
        self.pending.is_some()
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum KeymapResult {
    Pending,
    Action(ApplicationAction),
    Unmatched,
}

const fn chord(key: Key, modifiers: Modifiers) -> KeyChord {
    KeyChord { key, modifiers }
}

// Prototype bindings live together here so experiments do not leak into event handling.
pub const KEY_BINDINGS: &[KeyBinding] = &[
    KeyBinding {
        context: InputContext::Composer,
        first: chord(Key::Character('v'), Modifiers::CONTROL),
        second: None,
        action: ApplicationAction::Paste,
    },
    KeyBinding {
        context: InputContext::Composer,
        first: chord(Key::Character('h'), Modifiers::CONTROL),
        second: None,
        action: ApplicationAction::FocusHistory,
    },
    KeyBinding {
        context: InputContext::History,
        first: chord(Key::Escape, Modifiers::NONE),
        second: None,
        action: ApplicationAction::FocusComposer,
    },
    KeyBinding {
        context: InputContext::History,
        first: chord(Key::ArrowUp, Modifiers::NONE),
        second: None,
        action: ApplicationAction::MoveHistoryFocus(HistoryDirection::Older),
    },
    KeyBinding {
        context: InputContext::History,
        first: chord(Key::Character('k'), Modifiers::NONE),
        second: None,
        action: ApplicationAction::MoveHistoryFocus(HistoryDirection::Older),
    },
    KeyBinding {
        context: InputContext::History,
        first: chord(Key::ArrowDown, Modifiers::NONE),
        second: None,
        action: ApplicationAction::MoveHistoryFocus(HistoryDirection::Newer),
    },
    KeyBinding {
        context: InputContext::History,
        first: chord(Key::Character('j'), Modifiers::NONE),
        second: None,
        action: ApplicationAction::MoveHistoryFocus(HistoryDirection::Newer),
    },
    KeyBinding {
        context: InputContext::History,
        first: chord(Key::Space, Modifiers::NONE),
        second: None,
        action: ApplicationAction::Command(CommandAction::ToggleExpansion),
    },
    KeyBinding {
        context: InputContext::History,
        first: chord(Key::Enter, Modifiers::NONE),
        second: None,
        action: ApplicationAction::Command(CommandAction::Rerun),
    },
    KeyBinding {
        context: InputContext::History,
        first: chord(Key::Character('y'), Modifiers::NONE),
        second: Some(chord(Key::Character('y'), Modifiers::NONE)),
        action: ApplicationAction::Command(CommandAction::Copy(CopyScope::All)),
    },
    KeyBinding {
        context: InputContext::History,
        first: chord(Key::Character('y'), Modifiers::NONE),
        second: Some(chord(Key::Character('i'), Modifiers::NONE)),
        action: ApplicationAction::Command(CommandAction::Copy(CopyScope::Input)),
    },
    KeyBinding {
        context: InputContext::History,
        first: chord(Key::Character('y'), Modifiers::NONE),
        second: Some(chord(Key::Character('o'), Modifiers::NONE)),
        action: ApplicationAction::Command(CommandAction::Copy(CopyScope::Output)),
    },
];

#[must_use]
pub fn resolve_key_sequence(
    context: InputContext,
    state: KeySequenceState,
    pressed: KeyChord,
) -> (KeySequenceState, KeymapResult) {
    let completed = state.pending.and_then(|first| {
        KEY_BINDINGS
            .iter()
            .find(|binding| {
                binding.context == context
                    && binding.first == first
                    && binding.second == Some(pressed)
            })
            .map(|binding| binding.action)
    });
    if let Some(action) = completed {
        return (KeySequenceState::default(), KeymapResult::Action(action));
    }

    let single = KEY_BINDINGS
        .iter()
        .find(|binding| {
            binding.context == context && binding.first == pressed && binding.second.is_none()
        })
        .map(|binding| binding.action);
    if let Some(action) = single {
        return (KeySequenceState::default(), KeymapResult::Action(action));
    }

    if KEY_BINDINGS.iter().any(|binding| {
        binding.context == context && binding.first == pressed && binding.second.is_some()
    }) {
        return (
            KeySequenceState {
                pending: Some(pressed),
            },
            KeymapResult::Pending,
        );
    }

    (KeySequenceState::default(), KeymapResult::Unmatched)
}

#[must_use]
pub fn adjacent_command(
    commands: &[Command],
    focused: CommandId,
    direction: HistoryDirection,
) -> Option<CommandId> {
    let index = commands.iter().position(|command| command.id == focused)?;
    let adjacent_index = match direction {
        HistoryDirection::Older => index.saturating_sub(1),
        HistoryDirection::Newer => (index + 1).min(commands.len().saturating_sub(1)),
    };

    commands.get(adjacent_index).map(|command| command.id)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bindings_are_scoped_to_the_active_context() {
        let j = chord(Key::Character('j'), Modifiers::NONE);

        assert_eq!(
            resolve_key_sequence(InputContext::History, KeySequenceState::default(), j).1,
            KeymapResult::Action(ApplicationAction::MoveHistoryFocus(HistoryDirection::Newer))
        );
        assert_eq!(
            resolve_key_sequence(InputContext::Composer, KeySequenceState::default(), j).1,
            KeymapResult::Unmatched
        );
    }

    #[test]
    fn modifiers_are_part_of_a_binding() {
        let plain_h = chord(Key::Character('h'), Modifiers::NONE);
        let control_h = chord(Key::Character('h'), Modifiers::CONTROL);

        assert_eq!(
            resolve_key_sequence(InputContext::Composer, KeySequenceState::default(), plain_h).1,
            KeymapResult::Unmatched
        );
        assert_eq!(
            resolve_key_sequence(
                InputContext::Composer,
                KeySequenceState::default(),
                control_h
            )
            .1,
            KeymapResult::Action(ApplicationAction::FocusHistory)
        );
    }

    #[test]
    fn paste_is_scoped_to_the_composer() {
        let control_v = chord(Key::Character('v'), Modifiers::CONTROL);

        assert_eq!(
            resolve_key_sequence(
                InputContext::Composer,
                KeySequenceState::default(),
                control_v
            )
            .1,
            KeymapResult::Action(ApplicationAction::Paste)
        );
        assert_eq!(
            resolve_key_sequence(
                InputContext::History,
                KeySequenceState::default(),
                control_v
            )
            .1,
            KeymapResult::Unmatched
        );
    }

    #[test]
    fn copy_bindings_are_two_key_sequences() {
        let y = chord(Key::Character('y'), Modifiers::NONE);
        [
            ('y', CopyScope::All),
            ('i', CopyScope::Input),
            ('o', CopyScope::Output),
        ]
        .into_iter()
        .for_each(|(second, scope)| {
            let (pending, first_result) =
                resolve_key_sequence(InputContext::History, KeySequenceState::default(), y);
            let (complete, second_result) = resolve_key_sequence(
                InputContext::History,
                pending,
                chord(Key::Character(second), Modifiers::NONE),
            );

            assert_eq!(first_result, KeymapResult::Pending);
            assert!(pending.is_pending());
            assert_eq!(complete, KeySequenceState::default());
            assert_eq!(
                second_result,
                KeymapResult::Action(ApplicationAction::Command(CommandAction::Copy(scope)))
            );
        });
    }

    #[test]
    fn an_unmatched_second_key_is_retried_as_a_first_key() {
        let y = chord(Key::Character('y'), Modifiers::NONE);
        let k = chord(Key::Character('k'), Modifiers::NONE);
        let (pending, _) =
            resolve_key_sequence(InputContext::History, KeySequenceState::default(), y);
        let (_, result) = resolve_key_sequence(InputContext::History, pending, k);

        assert_eq!(
            result,
            KeymapResult::Action(ApplicationAction::MoveHistoryFocus(HistoryDirection::Older))
        );
    }

    #[test]
    fn history_navigation_follows_command_age_and_stops_at_ends() {
        let session = crate::model::Session::prototype();

        assert_eq!(
            adjacent_command(&session.commands, CommandId(3), HistoryDirection::Older),
            Some(CommandId(2))
        );
        assert_eq!(
            adjacent_command(&session.commands, CommandId(0), HistoryDirection::Older),
            Some(CommandId(0))
        );
        assert_eq!(
            adjacent_command(&session.commands, CommandId(3), HistoryDirection::Newer),
            Some(CommandId(3))
        );
    }
}
