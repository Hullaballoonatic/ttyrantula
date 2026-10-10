use std::time::Duration;

use super::{Command, CommandId, CommandState, Metadata};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Composer {
    pub input: String,
    pub hint: String,
    pub working_directory: String,
    pub branch: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Session {
    pub commands: Vec<Command>,
    pub composer: Composer,
}

impl Session {
    #[must_use]
    pub fn prototype() -> Self {
        Self {
            commands: vec![
                Command {
                    id: CommandId(0),
                    input: "git status --short".into(),
                    output: " M src/renderer.rs\n?? src/layout.rs".into(),
                    metadata: Metadata {
                        working_directory: "~/Projects/ttyrantula".into(),
                        branch: Some("main".into()),
                    },
                    duration: Some(Duration::from_millis(84)),
                    state: CommandState::Completed { exit_status: 0 },
                },
                Command {
                    id: CommandId(1),
                    input: "cargo test".into(),
                    output: "running 7 tests\ntest layout::tests::history_stays_above_composer ... ok\ntest layout::tests::hit_testing_finds_command ... ok\n\ntest result: ok. 7 passed; 0 failed".into(),
                    metadata: Metadata {
                        working_directory: "~/Projects/ttyrantula".into(),
                        branch: Some("main".into()),
                    },
                    duration: Some(Duration::from_millis(2_841)),
                    state: CommandState::Completed { exit_status: 0 },
                },
                Command {
                    id: CommandId(2),
                    input: "rg \"alternate_screen\" src".into(),
                    output: "No matches found".into(),
                    metadata: Metadata {
                        working_directory: "~/Projects/ttyrantula".into(),
                        branch: Some("main".into()),
                    },
                    duration: Some(Duration::from_millis(19)),
                    state: CommandState::Completed { exit_status: 1 },
                },
                Command {
                    id: CommandId(3),
                    input: "cargo build --release".into(),
                    output: "   Compiling wgpu-core v30.0.1\n   Compiling glyphon v0.12.0\n   Compiling ttyrantula v0.1.0".into(),
                    metadata: Metadata {
                        working_directory: "~/Projects/ttyrantula".into(),
                        branch: Some("main".into()),
                    },
                    duration: None,
                    state: CommandState::Running,
                },
            ],
            composer: Composer {
                hint: String::new(),
                input: "cargo test --".into(),
                working_directory: "~/Projects/ttyrantula".into(),
                branch: Some("main".into()),
            },
        }
    }

    #[must_use]
    pub fn with_composer_text(&self, text: &str) -> Self {
        let mut input = self.composer.input.clone();
        input.extend(text.chars().filter(|character| !character.is_control()));

        Self {
            composer: Composer {
                input,
                ..self.composer.clone()
            },
            ..self.clone()
        }
    }

    #[must_use]
    pub fn with_composer_backspace(&self) -> Self {
        let mut input = self.composer.input.clone();
        input.pop();

        Self {
            composer: Composer {
                input,
                ..self.composer.clone()
            },
            ..self.clone()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn composer_edits_return_updated_sessions() {
        let session = Session::prototype();
        let appended = session.with_composer_text("release\n");
        let deleted = appended.with_composer_backspace();

        assert_eq!(session.composer.input, "cargo test --");
        assert_eq!(appended.composer.input, "cargo test --release");
        assert_eq!(deleted.composer.input, "cargo test --releas");
    }
}
