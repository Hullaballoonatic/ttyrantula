//! Synchronous ownership boundary between a PTY and the terminal core.
//!
//! The application can later move PTY reads to an I/O worker without changing
//! terminal ownership: all bytes still enter `TerminalCore::write` through
//! this boundary.

use crate::{
    pty::{ExitStatus, PtyError, PtySession, Size},
    terminal::{GridSize, Snapshot, TerminalCore, TerminalError},
};
use std::{
    sync::{
        Arc, Mutex,
        mpsc::{self, Receiver, RecvTimeoutError, Sender},
    },
    thread,
    time::Duration,
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

/// Commands sent by the GUI/application owner to the terminal worker.
#[derive(Debug)]
pub enum TerminalCommand {
    Input(Vec<u8>),
    Resize(Size),
    Shutdown,
}

/// Events published by the terminal worker for the GUI/application owner.
#[derive(Debug)]
pub enum TerminalEvent {
    ChildExited(ExitStatus),
    Error(SessionError),
}

/// A latest-frame mailbox for GUI consumers.
#[derive(Clone, Debug, Default)]
pub struct SnapshotMailbox {
    latest: Arc<Mutex<Option<Snapshot>>>,
}

impl SnapshotMailbox {
    /// Takes the newest frame, dropping any older frame that was superseded.
    pub fn take_latest(&self) -> Option<Snapshot> {
        self.latest.lock().ok()?.take()
    }

    fn publish(&self, snapshot: Snapshot) {
        if let Ok(mut latest) = self.latest.lock() {
            *latest = Some(snapshot);
        }
    }
}

/// Handle for sending commands without exposing PTY or Ghostty state.
#[derive(Clone, Debug)]
pub struct TerminalHandle {
    commands: Sender<TerminalCommand>,
}

impl TerminalHandle {
    /// Sends bytes to the child process.
    pub fn input(&self, bytes: impl Into<Vec<u8>>) -> Result<(), SessionError> {
        self.commands
            .send(TerminalCommand::Input(bytes.into()))
            .map_err(|error| SessionError::Pty(PtyError::Child(error.to_string())))
    }

    /// Resizes both the PTY and terminal grid.
    pub fn resize(&self, size: Size) -> Result<(), SessionError> {
        self.commands
            .send(TerminalCommand::Resize(size))
            .map_err(|error| SessionError::Pty(PtyError::Child(error.to_string())))
    }

    /// Requests an orderly worker shutdown.
    pub fn shutdown(&self) -> Result<(), SessionError> {
        self.commands
            .send(TerminalCommand::Shutdown)
            .map_err(|error| SessionError::Pty(PtyError::Child(error.to_string())))
    }
}

/// Starts the terminal-core owner and returns its command/event channels.
pub fn spawn_worker(
    program: &str,
    size: Size,
) -> Result<(TerminalHandle, SnapshotMailbox, Receiver<TerminalEvent>), SessionError> {
    let (reader, mut control) = PtySession::spawn(program, size)?.split();
    let output = reader.spawn();
    let (commands, command_receiver) = mpsc::channel();
    let (event_sender, events) = mpsc::channel();
    let snapshots = SnapshotMailbox::default();
    let worker_snapshots = snapshots.clone();

    thread::spawn(move || {
        let grid_size = match GridSize::new(size.columns, size.rows) {
            Ok(grid_size) => grid_size,
            Err(error) => {
                let _ = event_sender.send(TerminalEvent::Error(SessionError::Terminal(error)));
                return;
            }
        };
        let terminal = match TerminalCore::new(grid_size) {
            Ok(terminal) => terminal,
            Err(error) => {
                let _ = event_sender.send(TerminalEvent::Error(SessionError::Terminal(error)));
                return;
            }
        };
        terminal_worker(
            terminal,
            &mut control,
            output,
            command_receiver,
            worker_snapshots,
            event_sender,
        );
    });

    Ok((TerminalHandle { commands }, snapshots, events))
}

fn terminal_worker(
    mut terminal: TerminalCore,
    control: &mut crate::pty::PtyControl,
    output: Receiver<Result<Vec<u8>, PtyError>>,
    commands: Receiver<TerminalCommand>,
    snapshots: SnapshotMailbox,
    events: Sender<TerminalEvent>,
) {
    if publish_snapshot(&mut terminal, &snapshots, &events).is_err() {
        return;
    }

    loop {
        match commands.recv_timeout(Duration::from_millis(10)) {
            Ok(TerminalCommand::Input(bytes)) => {
                if publish_result(control.write_input(&bytes), &events).is_err() {
                    return;
                }
            }
            Ok(TerminalCommand::Resize(size)) => {
                if publish_result(control.resize(size), &events).is_err() {
                    return;
                }
                let grid_size = match GridSize::new(size.columns, size.rows) {
                    Ok(grid_size) => grid_size,
                    Err(error) => {
                        let _ = events.send(TerminalEvent::Error(SessionError::Terminal(error)));
                        return;
                    }
                };
                if publish_terminal_result(terminal.resize(grid_size), &events).is_err()
                    || publish_snapshot(&mut terminal, &snapshots, &events).is_err()
                {
                    return;
                }
            }
            Ok(TerminalCommand::Shutdown) | Err(RecvTimeoutError::Disconnected) => return,
            Err(RecvTimeoutError::Timeout) => {}
        }

        match output.try_recv() {
            Ok(Ok(bytes)) => {
                terminal.write(&bytes);
                if publish_snapshot(&mut terminal, &snapshots, &events).is_err() {
                    return;
                }
            }
            Ok(Err(error)) => {
                let _ = events.send(TerminalEvent::Error(SessionError::Pty(error)));
                return;
            }
            Err(mpsc::TryRecvError::Empty) => {}
            Err(mpsc::TryRecvError::Disconnected) => return,
        }

        match control.try_wait() {
            Ok(Some(status)) => {
                let _ = events.send(TerminalEvent::ChildExited(status));
                return;
            }
            Ok(None) => {}
            Err(error) => {
                let _ = events.send(TerminalEvent::Error(SessionError::Pty(error)));
                return;
            }
        }
    }
}

fn publish_snapshot(
    terminal: &mut TerminalCore,
    snapshots: &SnapshotMailbox,
    events: &Sender<TerminalEvent>,
) -> Result<(), ()> {
    terminal
        .snapshot()
        .map(|snapshot| snapshots.publish(snapshot))
        .map_err(|error| {
            let _ = events.send(TerminalEvent::Error(SessionError::Terminal(error)));
        })
}

fn publish_result<T>(
    result: Result<T, PtyError>,
    events: &Sender<TerminalEvent>,
) -> Result<(), ()> {
    result.map(|_| ()).map_err(|error| {
        let _ = events.send(TerminalEvent::Error(SessionError::Pty(error)));
    })
}

fn publish_terminal_result(
    result: Result<(), TerminalError>,
    events: &Sender<TerminalEvent>,
) -> Result<(), ()> {
    result.map_err(|error| {
        let _ = events.send(TerminalEvent::Error(SessionError::Terminal(error)));
    })
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

    #[cfg(unix)]
    #[test]
    fn worker_owns_terminal_core_and_publishes_snapshots() {
        let (handle, snapshots, events) = spawn_worker("sh", Size::new(80, 24).unwrap()).unwrap();
        handle
            .input(b"printf 'threaded-ok'; exit 4\n".to_vec())
            .unwrap();

        let deadline = Instant::now() + Duration::from_secs(3);
        let mut saw_output = false;
        let status = loop {
            assert!(Instant::now() < deadline, "worker did not finish");
            if let Some(snapshot) = snapshots.take_latest() {
                saw_output |= snapshot
                    .rows
                    .iter()
                    .map(crate::terminal::Row::text)
                    .any(|row| row.contains("threaded-ok"));
            }
            match events.recv_timeout(Duration::from_millis(100)) {
                Ok(TerminalEvent::ChildExited(status)) => break status,
                Ok(TerminalEvent::Error(error)) => panic!("worker error: {error}"),
                Err(RecvTimeoutError::Timeout) => continue,
                Err(RecvTimeoutError::Disconnected) => panic!("worker disconnected"),
            }
        };

        if let Some(snapshot) = snapshots.take_latest() {
            saw_output |= snapshot
                .rows
                .iter()
                .map(crate::terminal::Row::text)
                .any(|row| row.contains("threaded-ok"));
        }
        assert!(saw_output);
        assert_eq!(status.code, Some(4));
        assert!(!status.success);
    }
}
