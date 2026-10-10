use std::time::Duration;

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct CommandId(pub usize);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CopyScope {
    All,
    Input,
    Output,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CommandState {
    Running,
    Completed { exit_status: i32 },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Metadata {
    pub working_directory: String,
    pub branch: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Command {
    pub id: CommandId,
    pub input: String,
    pub output: String,
    pub metadata: Metadata,
    pub duration: Option<Duration>,
    pub state: CommandState,
}

impl Command {
    #[must_use]
    pub fn clipboard_text(&self, scope: CopyScope) -> String {
        match scope {
            CopyScope::Input => self.input.clone(),
            CopyScope::Output => self.output.clone(),
            CopyScope::All => {
                let metadata = [
                    Some(format!(
                        "Working directory: {}",
                        self.metadata.working_directory
                    )),
                    self.metadata
                        .branch
                        .as_ref()
                        .map(|branch| format!("Branch: {branch}")),
                ]
                .into_iter()
                .flatten()
                .collect::<Vec<_>>()
                .join("\n");

                [
                    Some(metadata),
                    Some(self.input.clone()),
                    (!self.output.is_empty()).then(|| self.output.clone()),
                ]
                .into_iter()
                .flatten()
                .collect::<Vec<_>>()
                .join("\n\n")
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn copy_scopes_produce_plain_text() {
        let command = &crate::model::Session::prototype().commands[0];

        assert_eq!(
            command.clipboard_text(CopyScope::Input),
            "git status --short"
        );
        assert_eq!(
            command.clipboard_text(CopyScope::Output),
            " M src/renderer.rs\n?? src/layout.rs"
        );
        assert_eq!(
            command.clipboard_text(CopyScope::All),
            "Working directory: ~/Projects/ttyrantula\nBranch: main\n\ngit status --short\n\n M src/renderer.rs\n?? src/layout.rs"
        );
    }
}
