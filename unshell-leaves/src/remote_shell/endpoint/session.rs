//! Per-hook remote shell session lifecycle.
//!
//! A session opens one PTY-backed shell process and translates protocol hook
//! traffic into stdin writes and stdout or stderr chunks. Close is intentionally
//! two-sided: the peer signals input completion with `end_hook`, while the local
//! side closes only after the child exits and the PTY reader drains.

use std::io::{self, Read, Write};
use std::process::Command;
use std::sync::mpsc::{self, Receiver, SyncSender, TryRecvError};
use std::thread;

use portable_pty::{CommandBuilder, ExitStatus, PtySize, native_pty_system};
use unshell::Procedure;
use unshell::protocol::tree::{IncomingData, OutgoingData, ProcedureEffect};

use super::RemoteShellEndpoint;
use super::errors::ShellLeafError;

/// Per-hook shell session created by the `open` procedure.
///
/// The procedure type is also the stored session type so the mapping between
/// one opening procedure and one live hook remains direct and visible.
#[derive(Procedure)]
#[procedure(leaf = RemoteShellEndpoint, name = "open")]
pub struct ProcedureOpen {
    /// Spawned PTY child process.
    pub(super) child: Box<dyn portable_pty::Child + Send>,
    /// Process-group leader used for Unix hangup and kill signaling.
    process_group_leader: Option<u32>,
    /// Buffered stdin bridge into the shell process.
    stdin_tx: Option<SyncSender<Vec<u8>>>,
    /// Buffered output stream read from the PTY.
    output_rx: Receiver<OutputEvent>,
    /// Hook return path for packets emitted by this session.
    return_path: Vec<String>,
    /// Hook identifier allocated by the caller.
    hook_id: u64,
    /// Procedure id bound to this shell hook.
    procedure_id: String,
    /// Whether the PTY reader has closed and drained.
    output_closed: bool,
    /// Observed child exit status, once known.
    pub(super) exit_status: Option<ExitStatus>,
    /// Whether this session already emitted its terminal local packet.
    pub(super) local_end_sent: bool,
}

/// One event forwarded from the PTY reader thread.
enum OutputEvent {
    Chunk(Vec<u8>),
    ReaderClosed,
}

impl ProcedureOpen {
    pub(super) fn spawn(
        return_path: Vec<String>,
        hook_id: u64,
        procedure_id: String,
    ) -> Result<Self, ShellLeafError> {
        let command = build_shell_command();
        let pty_system = native_pty_system();
        let pair = pty_system
            .openpty(PtySize {
                rows: 24,
                cols: 80,
                pixel_width: 0,
                pixel_height: 0,
            })
            .map_err(|error| io::Error::other(error.to_string()))?;

        let child = pair
            .slave
            .spawn_command(command)
            .map_err(|error| io::Error::other(error.to_string()))?;
        let process_group_leader = child.process_id();
        let stdin = pair
            .master
            .take_writer()
            .map_err(|error| io::Error::other(error.to_string()))?;
        let stdout = pair
            .master
            .try_clone_reader()
            .map_err(|error| io::Error::other(error.to_string()))?;

        let (stdin_tx, rx) = spawn_io_threads(stdin, stdout);

        Ok(Self {
            child,
            process_group_leader,
            stdin_tx: Some(stdin_tx),
            output_rx: rx,
            return_path,
            hook_id,
            procedure_id,
            output_closed: false,
            exit_status: None,
            local_end_sent: false,
        })
    }

    /// Builds one outgoing hook packet owned by this session.
    pub(super) fn packet(&self, data: Vec<u8>, end_hook: bool) -> OutgoingData {
        OutgoingData {
            dst_path: self.return_path.clone(),
            hook_id: self.hook_id,
            procedure_id: self.procedure_id.clone(),
            data,
            end_hook,
        }
    }

    /// Forces the underlying shell process to stop and records its exit status.
    pub(super) fn terminate(&mut self) -> Result<(), ShellLeafError> {
        self.stdin_tx.take();
        match self.child.try_wait()? {
            Some(status) => {
                self.exit_status = Some(status);
                Ok(())
            }
            None => {
                self.signal_process_group("-KILL");
                self.child
                    .kill()
                    .map_err(|error| io::Error::other(error.to_string()))?;
                self.exit_status = Some(
                    self.child
                        .wait()
                        .map_err(|error| io::Error::other(error.to_string()))?,
                );
                Ok(())
            }
        }
    }

    /// Drains any currently buffered PTY output into protocol packets.
    pub(super) fn drain_output(&mut self, outgoing: &mut Vec<OutgoingData>) {
        loop {
            match self.output_rx.try_recv() {
                Ok(OutputEvent::Chunk(bytes)) => outgoing.push(self.packet(bytes, false)),
                Ok(OutputEvent::ReaderClosed) => self.output_closed = true,
                Err(TryRecvError::Empty) => break,
                Err(TryRecvError::Disconnected) => {
                    self.output_closed = true;
                    break;
                }
            }
        }
    }

    /// Applies one inbound hook payload to the shell process.
    pub(super) fn on_data(
        &mut self,
        data: IncomingData,
    ) -> Result<ProcedureEffect, ShellLeafError> {
        if !data.message.data.is_empty() {
            let Some(stdin_tx) = self.stdin_tx.as_ref() else {
                return Ok(ProcedureEffect::default());
            };
            stdin_tx.try_send(data.message.data).map_err(|_| {
                io::Error::new(io::ErrorKind::WouldBlock, "shell stdin channel full")
            })?;
        }

        if !data.message.end_hook {
            return Ok(ProcedureEffect::default());
        }

        // Peer end means no more stdin from the caller. Keep the process alive so
        // buffered PTY output can drain through the normal poll path.
        self.stdin_tx.take();
        self.signal_process_group("-HUP");
        Ok(ProcedureEffect::default())
    }

    /// Polls the shell for locally-generated output.
    pub(super) fn poll(&mut self) -> Result<ProcedureEffect, ShellLeafError> {
        let mut outgoing = Vec::new();
        self.drain_output(&mut outgoing);

        if self.local_end_sent {
            return Ok(ProcedureEffect::outgoing(outgoing));
        }

        if self.exit_status.is_none() {
            self.exit_status = self
                .child
                .try_wait()
                .map_err(|error| io::Error::other(error.to_string()))?;
        }

        if self.exit_status.is_some() && !self.output_closed {
            self.signal_process_group("-KILL");
        }

        if self.exit_status.is_some() && self.output_closed {
            outgoing.push(self.packet(Vec::new(), true));
            self.local_end_sent = true;
            return Ok(ProcedureEffect::close(outgoing));
        }

        Ok(ProcedureEffect::outgoing(outgoing))
    }

    fn signal_process_group(&self, signal: &str) {
        #[cfg(unix)]
        if let Some(process_group_leader) = self.process_group_leader {
            let _ = Command::new("kill")
                .arg(signal)
                .arg(format!("-{}", process_group_leader))
                .status();
        }
    }
}

impl Drop for ProcedureOpen {
    fn drop(&mut self) {
        let _ = self.terminate();
    }
}

fn spawn_pipe_writer(mut stdin: Box<dyn Write + Send>, rx: Receiver<Vec<u8>>) {
    thread::spawn(move || {
        for bytes in rx {
            if stdin.write_all(&bytes).is_err() {
                break;
            }
            if stdin.flush().is_err() {
                break;
            }
        }
    });
}

fn build_shell_command() -> CommandBuilder {
    if cfg!(windows) {
        let mut command = CommandBuilder::new("cmd.exe");
        command.arg("/Q");
        command
    } else {
        let mut command = CommandBuilder::new("/bin/sh");
        command.arg("-i");
        command
    }
}

fn spawn_io_threads(
    stdin: Box<dyn Write + Send>,
    stdout: Box<dyn Read + Send>,
) -> (SyncSender<Vec<u8>>, Receiver<OutputEvent>) {
    let (stdin_tx, stdin_rx) = mpsc::sync_channel(64);
    let (tx, rx) = mpsc::sync_channel(64);
    spawn_pipe_writer(stdin, stdin_rx);
    spawn_pipe_reader(stdout, tx);
    (stdin_tx, rx)
}

fn spawn_pipe_reader<R>(mut reader: R, tx: mpsc::SyncSender<OutputEvent>)
where
    R: Read + Send + 'static,
{
    thread::spawn(move || {
        loop {
            let mut buffer = [0u8; 1024];
            match reader.read(&mut buffer) {
                Ok(0) => {
                    let _ = tx.send(OutputEvent::ReaderClosed);
                    break;
                }
                Ok(read_len) => {
                    if tx
                        .send(OutputEvent::Chunk(buffer[..read_len].to_vec()))
                        .is_err()
                    {
                        break;
                    }
                }
                Err(error) if error.kind() == io::ErrorKind::Interrupted => {}
                Err(error) => {
                    let _ = tx.send(OutputEvent::Chunk(
                        format!("shell pipe read error: {error}\n").into_bytes(),
                    ));
                    let _ = tx.send(OutputEvent::ReaderClosed);
                    break;
                }
            }
        }
    });
}
