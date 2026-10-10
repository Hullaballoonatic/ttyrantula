//! A small, synchronous PTY session boundary.
//!
//! This module owns PTY transport and child-process lifecycle only. Terminal
//! parsing and semantic command handling stay in their respective modules.

use std::io::{Read, Write};
use std::sync::mpsc::{self, Receiver};
use std::thread;

use portable_pty::{Child, CommandBuilder, MasterPty, PtySize, native_pty_system};

/// PTY dimensions, including optional pixel dimensions for terminal programs
/// that use them.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Size {
    pub columns: u16,
    pub rows: u16,
    pub pixel_width: u16,
    pub pixel_height: u16,
}

impl Size {
    /// Creates a non-empty cell grid with unspecified pixel dimensions.
    pub fn new(columns: u16, rows: u16) -> Result<Self, PtyError> {
        (columns > 0 && rows > 0)
            .then_some(Self {
                columns,
                rows,
                pixel_width: 0,
                pixel_height: 0,
            })
            .ok_or(PtyError::InvalidSize { columns, rows })
    }

    fn into_pty_size(self) -> PtySize {
        PtySize {
            rows: self.rows,
            cols: self.columns,
            pixel_width: self.pixel_width,
            pixel_height: self.pixel_height,
        }
    }
}

/// A child-process exit result that does not expose the PTY dependency.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ExitStatus {
    pub code: Option<u32>,
    pub success: bool,
}

/// Errors at the PTY-session boundary.
#[derive(Debug)]
pub enum PtyError {
    InvalidSize { columns: u16, rows: u16 },
    Transport(String),
    Child(String),
}

impl std::fmt::Display for PtyError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidSize { columns, rows } => {
                write!(
                    formatter,
                    "PTY size must be non-empty, got {columns}x{rows}"
                )
            }
            Self::Transport(error) => write!(formatter, "PTY transport failed: {error}"),
            Self::Child(error) => write!(formatter, "PTY child operation failed: {error}"),
        }
    }
}

impl std::error::Error for PtyError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::InvalidSize { .. } | Self::Transport(_) | Self::Child(_) => None,
        }
    }
}

/// A running child attached to a master PTY.
pub struct PtySession {
    master: Box<dyn MasterPty>,
    reader: Box<dyn Read + Send>,
    writer: Box<dyn Write + Send>,
    child: Box<dyn Child>,
}

/// The blocking PTY output side, suitable for a dedicated reader thread.
pub struct PtyReader {
    reader: Box<dyn Read + Send>,
}

/// PTY operations that do not block waiting for output.
pub struct PtyControl {
    master: Box<dyn MasterPty>,
    writer: Box<dyn Write + Send>,
    child: Box<dyn Child>,
}

impl PtySession {
    /// Spawns `program` attached to a native PTY.
    pub fn spawn(program: &str, size: Size) -> Result<Self, PtyError> {
        Self::spawn_with_args(program, &[], size)
    }

    /// Spawns a program with explicit arguments attached to a native PTY.
    pub fn spawn_with_args(program: &str, args: &[&str], size: Size) -> Result<Self, PtyError> {
        let mut command = CommandBuilder::new(program);
        command.args(args);
        let pty_system = native_pty_system();
        let pair = pty_system
            .openpty(size.into_pty_size())
            .map_err(|error| PtyError::Child(error.to_string()))?;
        let child = pair
            .slave
            .spawn_command(command)
            .map_err(|error| PtyError::Child(error.to_string()))?;
        drop(pair.slave);

        let reader = pair
            .master
            .try_clone_reader()
            .map_err(|error| PtyError::Transport(error.to_string()))?;
        let writer = pair
            .master
            .take_writer()
            .map_err(|error| PtyError::Transport(error.to_string()))?;

        Ok(Self {
            master: pair.master,
            reader,
            writer,
            child,
        })
    }

    /// Separates blocking output reads from control operations.
    pub fn split(self) -> (PtyReader, PtyControl) {
        (
            PtyReader {
                reader: self.reader,
            },
            PtyControl {
                master: self.master,
                writer: self.writer,
                child: self.child,
            },
        )
    }

    /// Reads the next available PTY bytes into `buffer`.
    pub fn read(&mut self, buffer: &mut [u8]) -> Result<usize, PtyError> {
        self.reader
            .read(buffer)
            .map_err(|error| PtyError::Transport(error.to_string()))
    }

    /// Writes terminal input bytes to the child.
    pub fn write_all(&mut self, bytes: &[u8]) -> Result<(), PtyError> {
        self.writer
            .write_all(bytes)
            .map_err(|error| PtyError::Transport(error.to_string()))
    }

    /// Flushes bytes already written to the child.
    pub fn flush(&mut self) -> Result<(), PtyError> {
        self.writer
            .flush()
            .map_err(|error| PtyError::Transport(error.to_string()))
    }

    /// Updates the child-visible PTY dimensions.
    pub fn resize(&self, size: Size) -> Result<(), PtyError> {
        self.master
            .resize(size.into_pty_size())
            .map_err(|error| PtyError::Transport(error.to_string()))
    }

    /// Polls the child without blocking.
    pub fn try_wait(&mut self) -> Result<Option<ExitStatus>, PtyError> {
        self.child
            .try_wait()
            .map_err(|error| PtyError::Child(error.to_string()))
            .map(|status| status.map(exit_status))
    }

    /// Waits for the child to exit.
    pub fn wait(&mut self) -> Result<ExitStatus, PtyError> {
        self.child
            .wait()
            .map(exit_status)
            .map_err(|error| PtyError::Child(error.to_string()))
    }
}

impl PtyReader {
    /// Starts a blocking reader and sends each output chunk through a channel.
    pub fn spawn(self) -> Receiver<Result<Vec<u8>, PtyError>> {
        let (sender, receiver) = mpsc::channel();
        thread::spawn(move || {
            let mut reader = self.reader;
            let mut buffer = [0; 4096];
            loop {
                match reader.read(&mut buffer) {
                    Ok(0) => break,
                    Ok(count) => {
                        if sender.send(Ok(buffer[..count].to_vec())).is_err() {
                            break;
                        }
                    }
                    Err(error) => {
                        let _ = sender.send(Err(PtyError::Transport(error.to_string())));
                        break;
                    }
                }
            }
        });
        receiver
    }
}

impl PtyControl {
    /// Writes terminal input to the child.
    pub fn write_input(&mut self, bytes: &[u8]) -> Result<(), PtyError> {
        self.writer
            .write_all(bytes)
            .map_err(|error| PtyError::Transport(error.to_string()))?;
        self.writer
            .flush()
            .map_err(|error| PtyError::Transport(error.to_string()))
    }

    /// Updates the child-visible PTY dimensions.
    pub fn resize(&self, size: Size) -> Result<(), PtyError> {
        self.master
            .resize(size.into_pty_size())
            .map_err(|error| PtyError::Transport(error.to_string()))
    }

    /// Polls the child without blocking.
    pub fn try_wait(&mut self) -> Result<Option<ExitStatus>, PtyError> {
        self.child
            .try_wait()
            .map_err(|error| PtyError::Child(error.to_string()))
            .map(|status| status.map(exit_status))
    }
}

fn exit_status(status: portable_pty::ExitStatus) -> ExitStatus {
    ExitStatus {
        code: Some(status.exit_code()),
        success: status.success(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{Duration, Instant};

    #[test]
    fn rejects_an_empty_pty() {
        assert!(Size::new(0, 24).is_err());
        assert!(Size::new(80, 0).is_err());
    }

    #[cfg(unix)]
    #[test]
    fn transports_bytes_resizes_and_observes_child_exit() {
        let mut session = PtySession::spawn("sh", Size::new(80, 24).unwrap()).unwrap();
        session
            .write_all(b"printf 'pty-ok'; exit 7\n")
            .and_then(|()| session.flush())
            .unwrap();
        session
            .resize(Size {
                columns: 100,
                rows: 30,
                pixel_width: 800,
                pixel_height: 600,
            })
            .unwrap();

        let deadline = Instant::now() + Duration::from_secs(3);
        let mut bytes = Vec::new();
        let mut buffer = [0; 256];
        let status = loop {
            match session.try_wait().unwrap() {
                Some(status) => break status,
                None if Instant::now() >= deadline => panic!("child did not exit"),
                None => {
                    let count = session.read(&mut buffer).unwrap();
                    bytes.extend_from_slice(&buffer[..count]);
                }
            }
        };

        assert!(
            bytes
                .windows(b"pty-ok".len())
                .any(|window| window == b"pty-ok")
        );
        assert_eq!(status.code, Some(7));
        assert!(!status.success);
    }

    #[cfg(unix)]
    #[test]
    fn reader_worker_does_not_block_control_writes() {
        let session = PtySession::spawn("sh", Size::new(80, 24).unwrap()).unwrap();
        let (reader, mut control) = session.split();
        let output = reader.spawn();
        control
            .write_input(b"printf 'worker-ok'; exit 0\n")
            .unwrap();

        let bytes = output
            .recv_timeout(Duration::from_secs(3))
            .unwrap()
            .unwrap();
        assert!(
            bytes
                .windows(b"worker-ok".len())
                .any(|window| window == b"worker-ok")
        );
        let deadline = Instant::now() + Duration::from_secs(3);
        let status = loop {
            if let Some(status) = control.try_wait().unwrap() {
                break status;
            }
            assert!(Instant::now() < deadline, "child did not exit");
            std::thread::yield_now();
        };
        assert_eq!(status.code, Some(0));
    }
}
