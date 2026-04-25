#[path = "support/remote_shell_common.rs"]
mod common;

use std::error::Error;
use std::io::{self, Read, Write};
use std::net::TcpStream;
use std::process::{Child, ChildStdin, Command, ExitStatus, Stdio};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, Sender};
use std::thread;
use std::time::Duration;

use unshell::protocol::tree::{Endpoint, Ingress, LocalEvent};

struct ShellSession {
    child: Child,
    stdin: Option<ChildStdin>,
    return_path: Vec<String>,
    hook_id: u64,
    procedure_id: String,
    readers_closed: usize,
    exit_status: Option<ExitStatus>,
}

enum OutputEvent {
    Chunk(Vec<u8>),
    ReaderClosed,
}

fn main() -> Result<(), Box<dyn Error>> {
    let mut stream = TcpStream::connect(common::LISTEN_ADDR)?;
    let frame_rx = common::spawn_frame_reader(stream.try_clone()?);
    let mut endpoint = common::build_agent_endpoint();
    let mut session: Option<ShellSession> = None;
    let mut output_rx: Option<Receiver<OutputEvent>> = None;

    println!("connected to controller at {}", common::LISTEN_ADDR);

    loop {
        match frame_rx.recv_timeout(Duration::from_millis(25)) {
            Ok(result) => {
                let frame = result?;
                let outcome = endpoint.receive(&Ingress::Parent, frame)?;
                if let Some(event) = common::pump_outcome(&mut stream, outcome)? {
                    handle_local_event(
                        &mut endpoint,
                        &mut stream,
                        &mut session,
                        &mut output_rx,
                        event,
                    )?;
                }
            }
            Err(RecvTimeoutError::Timeout) => {}
            Err(RecvTimeoutError::Disconnected) => break,
        }

        if let Some(rx) = output_rx.as_ref() {
            while let Ok(event) = rx.try_recv() {
                handle_shell_output(&mut endpoint, &mut stream, &mut session, event)?;
            }
        }

        if finalize_exited_shell(&mut endpoint, &mut stream, &mut session)? {
            output_rx = None;
        }
    }

    Ok(())
}

fn handle_local_event(
    endpoint: &mut unshell::protocol::tree::ProtocolEndpoint,
    stream: &mut TcpStream,
    session: &mut Option<ShellSession>,
    output_rx: &mut Option<Receiver<OutputEvent>>,
    event: LocalEvent,
) -> Result<(), Box<dyn Error>> {
    match event {
        LocalEvent::Call { header, message } => {
            let shell_leaf_name = common::shell_leaf_name();
            let start_procedure = common::shell_start_procedure();
            if header.dst_leaf.as_deref() != Some(shell_leaf_name.as_str())
                || message.procedure_id != start_procedure
            {
                return Ok(());
            }

            let Some(hook) = message.response_hook else {
                return Ok(());
            };

            let (new_session, rx) =
                start_shell(&hook.return_path, hook.hook_id, &message.procedure_id)?;
            *session = Some(new_session);
            *output_rx = Some(rx);

            let outcome = endpoint.send_data(
                hook.return_path,
                hook.hook_id,
                message.procedure_id,
                b"shell ready\n".to_vec(),
                false,
            )?;
            let _ = common::pump_outcome(stream, outcome)?;
        }
        LocalEvent::Data { message, .. } => {
            let Some(active_session) = session.as_mut() else {
                return Ok(());
            };

            if !message.data.is_empty() {
                let Some(stdin) = active_session.stdin.as_mut() else {
                    return Ok(());
                };
                stdin.write_all(&message.data)?;
                stdin.flush()?;
            }

            if message.end_hook {
                active_session.stdin.take();
            }
        }
        LocalEvent::Fault { message, .. } => {
            eprintln!(
                "controller reported protocol fault: 0x{:02X}",
                message.fault.0
            );
        }
    }

    Ok(())
}

fn handle_shell_output(
    endpoint: &mut unshell::protocol::tree::ProtocolEndpoint,
    stream: &mut TcpStream,
    session: &mut Option<ShellSession>,
    event: OutputEvent,
) -> Result<(), Box<dyn Error>> {
    let Some(active_session) = session.as_mut() else {
        return Ok(());
    };

    match event {
        OutputEvent::Chunk(bytes) => {
            let outcome = endpoint.send_data(
                active_session.return_path.clone(),
                active_session.hook_id,
                active_session.procedure_id.clone(),
                bytes,
                false,
            )?;
            let _ = common::pump_outcome(stream, outcome)?;
        }
        OutputEvent::ReaderClosed => {
            active_session.readers_closed += 1;
        }
    }

    Ok(())
}

fn finalize_exited_shell(
    endpoint: &mut unshell::protocol::tree::ProtocolEndpoint,
    stream: &mut TcpStream,
    session: &mut Option<ShellSession>,
) -> Result<bool, Box<dyn Error>> {
    let Some(active_session) = session.as_mut() else {
        return Ok(false);
    };

    if active_session.exit_status.is_none() {
        active_session.exit_status = active_session.child.try_wait()?;
    }

    let Some(exit_status) = active_session.exit_status else {
        return Ok(false);
    };
    if active_session.readers_closed < 2 {
        return Ok(false);
    }

    let summary = format!("shell exited with {exit_status}\n");
    let outcome = endpoint.send_data(
        active_session.return_path.clone(),
        active_session.hook_id,
        active_session.procedure_id.clone(),
        summary.into_bytes(),
        true,
    )?;
    let _ = common::pump_outcome(stream, outcome)?;
    *session = None;
    Ok(true)
}

fn start_shell(
    return_path: &[String],
    hook_id: u64,
    procedure_id: &str,
) -> io::Result<(ShellSession, Receiver<OutputEvent>)> {
    let mut command = if cfg!(windows) {
        let mut command = Command::new("cmd.exe");
        command.arg("/Q");
        command
    } else {
        let mut command = Command::new("/bin/sh");
        command.arg("-i");
        command
    };

    let mut child = command
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()?;

    let stdin = child
        .stdin
        .take()
        .ok_or_else(|| io::Error::other("failed to capture shell stdin"))?;
    let stdout = child
        .stdout
        .take()
        .ok_or_else(|| io::Error::other("failed to capture shell stdout"))?;
    let stderr = child
        .stderr
        .take()
        .ok_or_else(|| io::Error::other("failed to capture shell stderr"))?;

    let (tx, rx) = mpsc::channel();
    spawn_pipe_reader(stdout, tx.clone());
    spawn_pipe_reader(stderr, tx);

    Ok((
        ShellSession {
            child,
            stdin: Some(stdin),
            return_path: return_path.to_vec(),
            hook_id,
            procedure_id: procedure_id.to_owned(),
            readers_closed: 0,
            exit_status: None,
        },
        rx,
    ))
}

fn spawn_pipe_reader<R>(mut reader: R, tx: Sender<OutputEvent>)
where
    R: Read + Send + 'static,
{
    thread::spawn(move || {
        let mut buffer = [0u8; 1024];
        loop {
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
