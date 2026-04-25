use std::io::{self, Read, Write};
use std::process::Command;
use std::sync::mpsc::{self, Receiver, SyncSender, TryRecvError};
use std::thread;

use portable_pty::{CommandBuilder, ExitStatus, PtySize, native_pty_system};
use unshell::protocol::tree::{IncomingData, OutgoingData, ProcedureEffect};

use unshell::Procedure;

use super::errors::ShellLeafError;

/// Per-hook shell session created by the `open` procedure.
///
/// The procedure type is also the stored session type. This keeps the mapping
/// between protocol procedure and hook state direct and easy to inspect.
#[derive(Procedure)]
#[procedure(leaf = RemoteShellLeaf, name = "open")]
pub struct ProcedureOpen {
    pub(super) child: Box<dyn portable_pty::Child + Send>,
    process_group_leader: Option<u32>,
    stdin_tx: Option<SyncSender<Vec<u8>>>,
    output_rx: Receiver<OutputEvent>,
    return_path: Vec<String>,
    hook_id: u64,
    procedure_id: String,
    output_closed: bool,
    pub(super) exit_status: Option<ExitStatus>,
    pub(super) local_end_sent: bool,
}

enum OutputEvent {
    Chunk(Vec<u8>),
    ReaderClosed,
}

use super::RemoteShellLeaf;

impl ProcedureOpen {
    pub(super) fn spawn(
        return_path: Vec<String>,
        hook_id: u64,
        procedure_id: String,
    ) -> Result<Self, ShellLeafError> {
        let pty_system = native_pty_system();
        let pair = pty_system
            .openpty(PtySize {
                rows: 24,
                cols: 80,
                pixel_width: 0,
                pixel_height: 0,
            })
            .map_err(|error| io::Error::other(error.to_string()))?;

        let command = if cfg!(windows) {
            let mut command = CommandBuilder::new("cmd.exe");
            command.arg("/Q");
            command
        } else {
            let mut command = CommandBuilder::new("/bin/sh");
            command.arg("-i");
            command
        };

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

        let (stdin_tx, stdin_rx) = mpsc::sync_channel(64);
        let (tx, rx) = mpsc::sync_channel(64);
        spawn_pipe_writer(stdin, stdin_rx);
        spawn_pipe_reader(stdout, tx);

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

    pub(super) fn packet(&self, data: Vec<u8>, end_hook: bool) -> OutgoingData {
        OutgoingData {
            dst_path: self.return_path.clone(),
            hook_id: self.hook_id,
            procedure_id: self.procedure_id.clone(),
            data,
            end_hook,
        }
    }

    pub(super) fn terminate(&mut self) -> Result<(), ShellLeafError> {
        self.stdin_tx.take();
        match self.child.try_wait()? {
            Some(status) => {
                self.exit_status = Some(status);
                Ok(())
            }
            None => {
                self.kill_process_group();
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
        // any buffered PTY output can drain through the normal poll path. On Unix
        // we also send SIGHUP so an interactive shell treats this like terminal
        // hangup instead of waiting forever on the still-open PTY master.
        self.stdin_tx.take();
        self.signal_peer_end();
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
            self.kill_process_group();
        }

        if self.exit_status.is_some() && self.output_closed {
            outgoing.push(self.packet(Vec::new(), true));
            self.local_end_sent = true;
            return Ok(ProcedureEffect::close(outgoing));
        }

        Ok(ProcedureEffect::outgoing(outgoing))
    }

    fn kill_process_group(&self) {
        #[cfg(unix)]
        if let Some(process_group_leader) = self.process_group_leader {
            let _ = Command::new("kill")
                .arg("-KILL")
                .arg(format!("-{}", process_group_leader))
                .status();
        }
    }

    fn signal_peer_end(&self) {
        #[cfg(unix)]
        if let Some(process_group_leader) = self.process_group_leader {
            let _ = Command::new("kill")
                .arg("-HUP")
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
