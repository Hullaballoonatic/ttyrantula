//! Synchronous ownership boundary between a PTY and the terminal core.
//!
//! The application can later move PTY reads to an I/O worker without changing
//! terminal ownership: all bytes still enter `TerminalCore::write` through
//! this boundary.

use crate::{
    pty::{ExitStatus, PtyError, PtySession, Size},
    terminal::{GridSize, Snapshot, TerminalCore, TerminalError},
};

/// Errors produced while routing PTY bytes through the terminal core.
#[derive(Debug)]
pub enum SessionError {
    Pty(PtyError),
    Terminal(TerminalError),
}

impl std::fmt::Display for SessionError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Pty(error) => write!(formatter, "{error}"),
            Self::Terminal(error) => write!(formatter, "{error}"),
        }
    }
}

impl std::error::Error for SessionError {}

impl From<PtyError> for SessionError {
    fn from(error: PtyError) -> Self {
        Self::Pty(error)
    }
}

impl From<TerminalError> for SessionError {
    fn from(error: TerminalError) -> Self {
        Self::Terminal(error)
    }
}

/// Owns one child PTY and the terminal state that interprets its output.
pub struct TerminalSession {
    pty: PtySession,
    terminal: TerminalCore,
}

impl TerminalSession {
    /// Starts `program` with a terminal core of the same dimensions.
    pub fn spawn(program: &str, size: Size) -> Result<Self, SessionError> {
        let terminal = TerminalCore::new(GridSize::new(size.columns, size.rows)?)?;
        let pty = PtySession::spawn(program, size)?;
        Ok(Self { pty, terminal })
    }

    /// Reads one PTY chunk and immediately feeds it to Ghostty.
    pub fn read_into_terminal(&mut self, buffer: &mut [u8]) -> Result<usize, SessionError> {
        let count = self.pty.read(buffer)?;
        self.terminal.write(&buffer[..count]);
        Ok(count)
    }

    /// Encodes application input as bytes sent to the child process.
    pub fn write_input(&mut self, bytes: &[u8]) -> Result<(), SessionError> {
        self.pty.write_all(bytes)?;
        self.pty.flush()?;
        Ok(())
    }

    /// Resizes both the PTY and Ghostty's terminal grid.
    pub fn resize(&mut self, size: Size) -> Result<(), SessionError> {
        self.pty.resize(size)?;
        self.terminal
            .resize(GridSize::new(size.columns, size.rows)?)?;
        Ok(())
    }

    /// Polls the child process without blocking.
    pub fn try_wait(&mut self) -> Result<Option<ExitStatus>, SessionError> {
        Ok(self.pty.try_wait()?)
    }

    /// Captures the current terminal frame as owned application data.
    pub fn snapshot(&mut self) -> Result<Snapshot, SessionError> {
        Ok(self.terminal.snapshot()?)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{Duration, Instant};

    #[cfg(unix)]
    #[test]
    fn routes_shell_output_into_terminal_snapshot() {
        let size = Size::new(80, 24).unwrap();
        let mut session = TerminalSession::spawn("sh", size).unwrap();
        session
            .write_input(b"printf '\\033[31mghostty-ok\\033[0m'; exit 3\n")
            .unwrap();

        let deadline = Instant::now() + Duration::from_secs(3);
        let mut buffer = [0; 256];
        let status = loop {
            if let Some(status) = session.try_wait().unwrap() {
                break status;
            }
            assert!(Instant::now() < deadline, "child did not exit");
            session.read_into_terminal(&mut buffer).unwrap();
        };

        let snapshot = session.snapshot().unwrap();
        let text = snapshot
            .rows
            .iter()
            .map(crate::terminal::Row::text)
            .collect::<Vec<_>>()
            .join("\n");
        assert!(text.contains("ghostty-ok"));
        assert_eq!(status.code, Some(3));
        assert!(!status.success);
    }
}
