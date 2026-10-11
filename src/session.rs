//! Threaded PTY and terminal-core ownership, semantic events, and frame delivery.

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
enum TerminalCommand {
    Input(Vec<u8>),
    Rerun(String),
    Resize(Size),
    Shutdown,
}

/// Events published by the terminal worker for the GUI/application owner.
#[derive(Debug)]
pub enum TerminalEvent {
    WorkingDirectory(String),
    CommandStarted {
        input: String,
    },
    CommandOutput {
        output: String,
    },
    CommandCompleted {
        output: String,
        exit_status: Option<i32>,
    },
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
    /// Replaces the editable line and executes it in the current shell directory.
    pub fn rerun(&self, input: String) -> Result<(), SessionError> {
        if input.contains(['\n', '\r', '\x1b']) {
            return Err(PtyError::Child("multiline rerun is not supported yet".into()).into());
        }
        self.commands
            .send(TerminalCommand::Rerun(input))
            .map_err(|error| SessionError::Pty(PtyError::Child(error.to_string())))
    }
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
    let history_home = if program == "nu" {
        Some(IsolatedHistory::new()?)
    } else {
        None
    };
    let config_home = history_home
        .as_ref()
        .map(|history| history.0.to_string_lossy().into_owned())
        .unwrap_or_default();
    let args: &[&str] = if program == "nu" {
        &[
            "--no-config-file",
            "--config-home",
            &config_home,
            "--execute",
            "$env.config.use_ansi_coloring = true; $env.config.show_hints = true; $env.config.color_config.hints = {fg: '#808080', attr: d}",
        ]
    } else {
        &[]
    };
    let (reader, mut control) = PtySession::spawn_with_args(program, args, size)?.split();
    let output = reader.spawn();
    let (commands, command_receiver) = mpsc::channel();
    let (event_sender, events) = mpsc::channel();
    let snapshots = SnapshotMailbox::default();
    let worker_snapshots = snapshots.clone();

    thread::spawn(move || {
        let _history_home = history_home;
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

struct IsolatedHistory(std::path::PathBuf);

impl IsolatedHistory {
    fn new() -> Result<Self, PtyError> {
        use std::sync::atomic::{AtomicU64, Ordering};
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let path = std::env::temp_dir().join(format!(
            "ttyrantula-history-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir(&path).map_err(|error| PtyError::Child(error.to_string()))?;
        Ok(Self(path))
    }
}

impl Drop for IsolatedHistory {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(self.0.join("history.txt"));
        let _ = std::fs::remove_dir(&self.0);
    }
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
    let mut child_status = None;
    let mut decoder = crate::semantic::SemanticDecoder::default();
    let mut command_terminal: Option<TerminalCore> = None;
    let mut command_input = String::new();
    let mut pending_rerun: Option<(String, std::time::Instant)> = None;

    loop {
        match commands.recv_timeout(Duration::from_millis(10)) {
            Ok(TerminalCommand::Rerun(input)) => {
                if command_terminal.is_some()
                    || pending_rerun.is_some()
                    || terminal
                        .snapshot()
                        .ok()
                        .and_then(|s| s.composer_input())
                        .is_none()
                {
                    continue;
                }
                let bytes = format!("\x01\x0b\x1b[200~{input}\x1b[201~");
                if publish_result(control.write_input(bytes.as_bytes()), &events).is_err() {
                    return;
                }
                pending_rerun = Some((input, std::time::Instant::now()));
            }
            Ok(TerminalCommand::Input(bytes)) => {
                if let Some(input) = terminal.snapshot().ok().and_then(|s| s.composer_input()) {
                    command_input = input;
                }
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
                if let Some(input) = terminal.snapshot().ok().and_then(|s| s.composer_input()) {
                    command_input = input;
                }
                for byte in bytes {
                    terminal.write(&[byte]);
                    if let Some(capture) = &mut command_terminal {
                        capture.write(&[byte]);
                    }
                    for event in decoder.feed(&[byte]) {
                        match event {
                            crate::semantic::SemanticEvent::WorkingDirectory(path) => {
                                let _ = events.send(TerminalEvent::WorkingDirectory(path));
                            }
                            crate::semantic::SemanticEvent::OutputStarted => {
                                if let Ok(snapshot) = terminal.snapshot() {
                                    command_terminal = TerminalCore::new(snapshot.size).ok();
                                }
                                let _ = events.send(TerminalEvent::CommandStarted {
                                    input: command_input.clone(),
                                });
                            }
                            crate::semantic::SemanticEvent::CommandCompleted { exit_status } => {
                                if let Some(mut capture) = command_terminal.take() {
                                    let _ = events.send(TerminalEvent::CommandCompleted {
                                        output: captured_text(&mut capture),
                                        exit_status,
                                    });
                                }
                            }
                            _ => {}
                        }
                    }
                }
                if let Some(capture) = &mut command_terminal {
                    let _ = events.send(TerminalEvent::CommandOutput {
                        output: captured_text(capture),
                    });
                }
                let replies = terminal.take_replies();
                if !replies.is_empty()
                    && publish_result(control.write_input(&replies), &events).is_err()
                {
                    return;
                }
                if publish_snapshot(&mut terminal, &snapshots, &events).is_err() {
                    return;
                }
            }
            Ok(Err(error)) => {
                let _ = events.send(TerminalEvent::Error(SessionError::Pty(error)));
                return;
            }
            Err(mpsc::TryRecvError::Empty) => {}
            Err(mpsc::TryRecvError::Disconnected) => {
                if let Some(status) = child_status {
                    let _ = events.send(TerminalEvent::ChildExited(status));
                    return;
                }
            }
        }

        if let Some((input, started)) = &pending_rerun {
            if terminal
                .snapshot()
                .ok()
                .and_then(|s| s.composer_input())
                .as_ref()
                == Some(input)
            {
                command_input = input.clone();
                if publish_result(control.write_input(b"\r"), &events).is_err() {
                    return;
                }
                pending_rerun = None;
            } else if started.elapsed() > Duration::from_secs(3) {
                let _ = events.send(TerminalEvent::Error(
                    PtyError::Child("shell did not confirm rerun input".into()).into(),
                ));
                pending_rerun = None;
            }
        }

        match control.try_wait() {
            Ok(Some(status)) => {
                child_status = Some(status);
            }
            Ok(None) => {}
            Err(error) => {
                let _ = events.send(TerminalEvent::Error(SessionError::Pty(error)));
                return;
            }
        }
    }
}

fn captured_text(terminal: &mut TerminalCore) -> String {
    terminal
        .snapshot()
        .map(|snapshot| {
            snapshot
                .rows
                .iter()
                .map(|row| row.text().trim_end().to_owned())
                .collect::<Vec<_>>()
                .join("\n")
                .trim_end()
                .to_owned()
        })
        .unwrap_or_default()
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{Duration, Instant};

    #[test]
    fn rerun_replaces_draft_and_records_exact_command() {
        let (handle, snapshots, events) = spawn_worker("nu", Size::new(80, 24).unwrap()).unwrap();
        let deadline = Instant::now() + Duration::from_secs(5);
        loop {
            if snapshots
                .take_latest()
                .and_then(|s| s.composer_input())
                .is_some()
            {
                break;
            }
            assert!(Instant::now() < deadline);
            thread::sleep(Duration::from_millis(20));
        }
        handle.input(b"draft not to execute".to_vec()).unwrap();
        loop {
            if snapshots
                .take_latest()
                .and_then(|s| s.composer_input())
                .as_deref()
                == Some("draft not to execute")
            {
                break;
            }
            assert!(Instant::now() < deadline);
            thread::sleep(Duration::from_millis(20));
        }
        handle.rerun("print rerun-visible".into()).unwrap();
        let mut started = false;
        loop {
            match events.recv_timeout(Duration::from_secs(4)).unwrap() {
                TerminalEvent::CommandStarted { input } => {
                    assert_eq!(input, "print rerun-visible");
                    started = true;
                }
                TerminalEvent::CommandCompleted {
                    output,
                    exit_status,
                } => {
                    assert!(started);
                    assert_eq!(output, "rerun-visible");
                    assert_eq!(exit_status, Some(0));
                    break;
                }
                TerminalEvent::Error(error) => panic!("{error}"),
                _ => {}
            }
        }
        handle.input(b"exit\r".to_vec()).unwrap();
    }

    #[test]
    fn nushell_reports_changed_directory() {
        let (handle, snapshots, events) = spawn_worker("nu", Size::new(80, 24).unwrap()).unwrap();
        let deadline = Instant::now() + Duration::from_secs(5);
        while snapshots
            .take_latest()
            .and_then(|s| s.composer_input())
            .is_none()
        {
            assert!(Instant::now() < deadline);
            thread::sleep(Duration::from_millis(20));
        }
        while events.try_recv().is_ok() {}
        handle.input(b"cd /\r".to_vec()).unwrap();
        loop {
            assert!(Instant::now() < deadline);
            match events.recv_timeout(Duration::from_secs(3)).unwrap() {
                TerminalEvent::WorkingDirectory(path) if path == "/" => break,
                TerminalEvent::Error(error) => panic!("{error}"),
                _ => {}
            }
        }
        handle.input(b"exit\r".to_vec()).unwrap();
    }

    #[test]
    fn nushell_command_output_has_completion_status() {
        let (handle, snapshots, events) = spawn_worker("nu", Size::new(80, 24).unwrap()).unwrap();
        let deadline = Instant::now() + Duration::from_secs(5);
        loop {
            if snapshots
                .take_latest()
                .and_then(|s| s.composer_input())
                .is_some()
            {
                break;
            }
            assert!(Instant::now() < deadline);
            thread::sleep(Duration::from_millis(20));
        }
        handle.input(b"print history-visible".to_vec()).unwrap();
        loop {
            if snapshots
                .take_latest()
                .and_then(|s| s.composer_input())
                .as_deref()
                == Some("print history-visible")
            {
                break;
            }
            assert!(Instant::now() < deadline);
            thread::sleep(Duration::from_millis(20));
        }
        handle.input(b"\r".to_vec()).unwrap();
        let mut started = false;
        loop {
            match events.recv_timeout(Duration::from_secs(3)).unwrap() {
                TerminalEvent::CommandStarted { input } => {
                    assert_eq!(input, "print history-visible");
                    started = true;
                }
                TerminalEvent::CommandCompleted {
                    output,
                    exit_status,
                } => {
                    assert!(started);
                    assert_eq!(output, "history-visible");
                    assert_eq!(exit_status, Some(0));
                    break;
                }
                TerminalEvent::Error(error) => panic!("{error}"),
                _ => {}
            }
        }
        handle.input(b"print h".to_vec()).unwrap();
        let deadline = Instant::now() + Duration::from_secs(3);
        let mut debug_input = String::new();
        loop {
            if let Some(snapshot) = snapshots.take_latest() {
                debug_input = format!(
                    "input={:?} hint={:?}",
                    snapshot.composer_input(),
                    snapshot.composer_hint()
                );
                if snapshot.composer_input().as_deref() == Some("print h")
                    && snapshot.composer_hint() == "istory-visible"
                {
                    break;
                }
            }
            assert!(Instant::now() < deadline, "hint missing {debug_input}");
            thread::sleep(Duration::from_millis(20));
        }
        handle.input(b"\x1b[C".to_vec()).unwrap();
        loop {
            if let Some(snapshot) = snapshots.take_latest()
                && snapshot.composer_input().as_deref() == Some("print history-visible")
                && snapshot.composer_hint().is_empty()
            {
                break;
            }
            assert!(Instant::now() < deadline, "hint not accepted");
            thread::sleep(Duration::from_millis(20));
        }
        handle.input(b"\r".to_vec()).unwrap();
        loop {
            match events.recv_timeout(Duration::from_secs(3)).unwrap() {
                TerminalEvent::CommandStarted { input } => {
                    assert_eq!(input, "print history-visible")
                }
                TerminalEvent::CommandCompleted {
                    output,
                    exit_status,
                } => {
                    assert_eq!(output, "history-visible");
                    assert_eq!(exit_status, Some(0));
                    break;
                }
                TerminalEvent::Error(error) => panic!("{error}"),
                _ => {}
            }
        }
        handle.input(b"exit\r".to_vec()).unwrap();
    }

    #[test]
    fn nushell_typing_is_visible_in_composer() {
        let (handle, snapshots, _) = spawn_worker("nu", Size::new(80, 24).unwrap()).unwrap();
        for (bytes, expected) in [
            (b"".as_slice(), ""),
            (b"l".as_slice(), "l"),
            (b"\x7f".as_slice(), ""),
            (b"p".as_slice(), "p"),
            (b"r".as_slice(), "pr"),
            (b"\x7f".as_slice(), "p"),
            (b" ".as_slice(), "p "),
            (b" ".as_slice(), "p  "),
            (b"\x7f\x7f".as_slice(), "p"),
            (
                b"rint composer-visible".as_slice(),
                "print composer-visible",
            ),
        ] {
            handle.input(bytes.to_vec()).unwrap();
            let deadline = Instant::now() + Duration::from_secs(3);
            let mut latest = None;
            loop {
                if let Some(snapshot) = snapshots.take_latest() {
                    latest = snapshot.composer_input();
                    if latest.as_deref() == Some(expected) {
                        break;
                    }
                }
                assert!(
                    Instant::now() < deadline,
                    "expected {expected:?}, got {latest:?}"
                );
                std::thread::sleep(Duration::from_millis(20));
            }
        }
        handle.input(b"\x03exit\r".to_vec()).unwrap();
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
                Ok(_) => {}
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

    #[cfg(unix)]
    #[test]
    fn worker_resize_updates_pty_and_terminal_grid() {
        let (handle, snapshots, events) = spawn_worker("sh", Size::new(80, 24).unwrap()).unwrap();
        handle
            .resize(Size {
                columns: 100,
                rows: 30,
                pixel_width: 800,
                pixel_height: 600,
            })
            .unwrap();

        let deadline = Instant::now() + Duration::from_secs(3);
        let resized = loop {
            assert!(Instant::now() < deadline, "worker did not publish resize");
            if let Some(snapshot) = snapshots.take_latest()
                && snapshot.size
                    == (crate::terminal::GridSize {
                        columns: 100,
                        rows: 30,
                    })
            {
                break true;
            }
            match events.recv_timeout(Duration::from_millis(20)) {
                Ok(TerminalEvent::Error(error)) => panic!("worker error: {error}"),
                Ok(TerminalEvent::ChildExited(_)) => break false,
                Err(RecvTimeoutError::Timeout) => {}
                Err(RecvTimeoutError::Disconnected) => break false,
                Ok(_) => {}
            }
        };
        handle.shutdown().unwrap();
        assert!(resized);
    }
}
