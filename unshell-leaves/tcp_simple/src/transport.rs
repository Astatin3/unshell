use std::{
    io::{self, Read, Write},
    net::TcpStream,
};

use unshell::protocol::{Endpoint, Packet};

#[cfg(target_os = "linux")]
const WOULD_BLOCK: i32 = 11;

/// Returns whether `error` is the expected nonblocking-socket retry signal.
///
/// Linux minimized endpoints use the raw `EAGAIN`/`EWOULDBLOCK` value to avoid
/// linking the broader `ErrorKind` classification path. Other targets keep the
/// portable standard-library classification because their raw values differ.
#[inline(always)]
fn is_would_block(error: &io::Error) -> bool {
    #[cfg(target_os = "linux")]
    {
        error.raw_os_error() == Some(WOULD_BLOCK)
    }

    #[cfg(not(target_os = "linux"))]
    {
        error.kind() == io::ErrorKind::WouldBlock
    }
}

/// Shared packet-to-TCP bridge used by the server and client leaves.
///
/// TCP is a byte stream, while the protocol serializer emits one self-delimiting
/// packet frame at a time. This helper keeps just enough buffering to rebuild full
/// frames from arbitrary reads, route them through the endpoint, and preserve
/// partially written outbound bytes across nonblocking update ticks.
#[derive(Debug)]
pub(crate) struct TcpBridge {
    remote_id: u32,
    is_authority: bool,
    stream: Option<TcpStream>,
    read_buffer: Vec<u8>,
    write_buffer: Vec<u8>,
    registered: bool,
}

impl TcpBridge {
    /// Creates bridge state for one adjacent endpoint.
    ///
    /// `is_authority` is passed directly to [`Endpoint::add_connection`]. Use `true`
    /// when the remote endpoint is the parent/authority and `false` when it is a
    /// child, matching the endpoint routing contract.
    pub(crate) fn new(remote_id: u32, is_authority: bool) -> Self {
        Self {
            remote_id,
            is_authority,
            stream: None,
            read_buffer: Vec::new(),
            write_buffer: Vec::new(),
            registered: false,
        }
    }

    /// Registers the transport edge once so endpoint routing accepts this peer.
    pub(crate) fn register(&mut self, endpoint: &mut Endpoint) {
        if !self.registered {
            endpoint.add_connection(self.remote_id, self.is_authority);
            self.registered = true;
        }
    }

    /// Returns whether there is an active TCP stream for this bridge.
    pub(crate) fn is_connected(&self) -> bool {
        self.stream.is_some()
    }

    /// Installs a newly connected stream and makes it nonblocking for update loops.
    ///
    /// Stale buffers are cleared before replacing the socket because a partial packet
    /// from an old TCP stream cannot be resumed safely on a new stream. TCP only gives
    /// byte ordering inside one connection, not across reconnects.
    pub(crate) fn set_stream(&mut self, stream: TcpStream) -> io::Result<()> {
        stream.set_nonblocking(true)?;
        self.read_buffer.clear();
        self.write_buffer.clear();
        self.stream = Some(stream);
        Ok(())
    }

    /// Moves all currently available TCP frames into the endpoint and flushes queued output.
    #[inline(never)]
    pub(crate) fn update(&mut self, endpoint: &mut Endpoint) {
        self.read_available();
        self.route_complete_frames(endpoint);

        if self.stream.is_none() {
            return;
        }

        self.collect_outbound(endpoint);
        self.flush_pending();
    }

    /// Reads until the nonblocking stream would block or disconnects.
    fn read_available(&mut self) {
        let Some(stream) = self.stream.as_mut() else {
            return;
        };

        let mut chunk = [0u8; 1024];

        loop {
            match stream.read(&mut chunk) {
                Ok(0) => {
                    self.disconnect();
                    break;
                }
                Ok(read) => self.read_buffer.extend_from_slice(&chunk[..read]),
                Err(error) if is_would_block(&error) => break,
                Err(_) => {
                    self.disconnect();
                    break;
                }
            }
        }
    }

    /// Routes each complete serialized packet frame currently buffered from TCP.
    fn route_complete_frames(&mut self, endpoint: &mut Endpoint) {
        while let Some(frame_len) = next_frame_len(&self.read_buffer) {
            // Transport input is untrusted. Bad frames and route failures are dropped
            // so a peer cannot wedge the bridge with one malformed packet.
            if let Ok(packet) = Packet::deserialize(&self.read_buffer[..frame_len]) {
                let _ = endpoint.add_inbound_from(self.remote_id, packet);
            }

            // `Packet::deserialize` owns the decoded path/data, so the byte frame can
            // be discarded after routing without allocating a second temporary buffer.
            self.read_buffer.copy_within(frame_len.., 0);
            self.read_buffer
                .truncate(self.read_buffer.len() - frame_len);
        }
    }

    /// Serializes endpoint packets queued for this remote into the pending write buffer.
    fn collect_outbound(&mut self, endpoint: &mut Endpoint) {
        let Some(queue) = endpoint.take_outbound_queue(self.remote_id) else {
            return;
        };

        for packet in queue {
            let _ = packet.serialize_into(&mut self.write_buffer);
        }
    }

    /// Writes pending bytes without blocking the endpoint loop.
    fn flush_pending(&mut self) {
        while !self.write_buffer.is_empty() {
            let Some(stream) = self.stream.as_mut() else {
                return;
            };

            match stream.write(&self.write_buffer) {
                Ok(0) => {
                    self.disconnect();
                    return;
                }
                Ok(written) => {
                    self.write_buffer.copy_within(written.., 0);
                    self.write_buffer
                        .truncate(self.write_buffer.len() - written);
                }
                Err(error) if is_would_block(&error) => return,
                Err(_) => {
                    self.disconnect();
                    return;
                }
            }
        }
    }

    /// Drops socket-local state; routing registration remains the intended topology.
    fn disconnect(&mut self) {
        self.stream = None;
        self.read_buffer.clear();
        self.write_buffer.clear();
    }
}

/// Returns the byte length of the next complete serialized packet in `buf`.
///
/// The packet format has no outer TCP length prefix, so the bridge derives the frame
/// boundary from `path_len` and `body_len`. `None` means either more bytes are needed
/// or the advertised lengths overflowed; in both cases the safest small transport
/// behavior is to wait rather than guess at packet boundaries.
fn next_frame_len(buf: &[u8]) -> Option<usize> {
    if buf.len() < 8 {
        return None;
    }

    let path_len = u32::from_le_bytes([buf[4], buf[5], buf[6], buf[7]]) as usize;
    let path_bytes = path_len.checked_mul(4)?;
    let body_len_offset = 8usize.checked_add(path_bytes)?;

    if buf.len() < body_len_offset.checked_add(4)? {
        return None;
    }

    let body_len = u32::from_le_bytes([
        buf[body_len_offset],
        buf[body_len_offset + 1],
        buf[body_len_offset + 2],
        buf[body_len_offset + 3],
    ]) as usize;

    let frame_len = body_len_offset.checked_add(4)?.checked_add(body_len)?;

    (buf.len() >= frame_len).then_some(frame_len)
}

#[cfg(test)]
mod tests {
    use std::{
        io::{Read, Write},
        net::{TcpListener, TcpStream},
        time::Duration,
    };

    use unshell::protocol::{Endpoint, Packet};

    use super::{TcpBridge, next_frame_len};

    const PARENT: u32 = 0x1000_0001;
    const CHILD: u32 = 0x1000_0002;
    const PROCEDURE: u32 = 0x2000_0001;

    /// Builds the parent side of the two-node topology used by bridge tests.
    ///
    /// The real endpoint constructor intentionally starts with an empty path so callers
    /// can attach it anywhere in the tree. Transport tests set the path explicitly to
    /// exercise the same routing contract production callers must satisfy.
    fn parent_endpoint() -> Endpoint {
        let mut endpoint = Endpoint::new(PARENT);
        endpoint.path = vec![PARENT];
        endpoint
    }

    /// Creates a local TCP pair without depending on a fixed port.
    fn connected_pair() -> (TcpStream, TcpStream) {
        let listener = TcpListener::bind(("127.0.0.1", 0)).unwrap();
        let addr = listener.local_addr().unwrap();
        let client = TcpStream::connect(addr).unwrap();
        let (server, _) = listener.accept().unwrap();

        client
            .set_read_timeout(Some(Duration::from_secs(1)))
            .unwrap();
        client
            .set_write_timeout(Some(Duration::from_secs(1)))
            .unwrap();

        (server, client)
    }

    /// Reads exactly one serialized packet frame from a blocking test stream.
    fn read_frame(stream: &mut TcpStream) -> Vec<u8> {
        let mut frame = Vec::new();
        let mut chunk = [0u8; 64];

        loop {
            let read = stream.read(&mut chunk).unwrap();
            assert_ne!(read, 0, "test TCP stream closed before a packet arrived");
            frame.extend_from_slice(&chunk[..read]);

            if let Some(frame_len) = next_frame_len(&frame) {
                assert_eq!(frame_len, frame.len());
                return frame;
            }
        }
    }

    /// Creates a downward packet that paves a return hook from parent to child.
    fn downward_packet(hook_id: u16) -> Packet {
        Packet {
            hook_id,
            end_hook: false,
            path: vec![PARENT, CHILD],
            procedure_id: PROCEDURE,
            data: vec![1, 2, 3],
        }
    }

    #[test]
    fn update_keeps_outbound_queued_until_connected() {
        let mut endpoint = parent_endpoint();
        let mut bridge = TcpBridge::new(CHILD, false);
        bridge.register(&mut endpoint);

        endpoint.add_outbound(downward_packet(7)).unwrap();
        bridge.update(&mut endpoint);

        let mut queued = 0usize;
        endpoint.take_outbound_clear(CHILD, |_| queued += 1);

        assert_eq!(queued, 1);
    }

    #[test]
    fn bridge_writes_outbound_and_routes_inbound_reply() {
        let mut endpoint = parent_endpoint();
        let mut bridge = TcpBridge::new(CHILD, false);
        let (server, mut client) = connected_pair();
        bridge.register(&mut endpoint);
        bridge.set_stream(server).unwrap();

        endpoint.add_outbound(downward_packet(9)).unwrap();
        bridge.update(&mut endpoint);

        let sent = Packet::deserialize(&read_frame(&mut client)).unwrap();
        assert_eq!(sent.hook_id, 9);
        assert_eq!(sent.path, vec![PARENT, CHILD]);
        assert_eq!(sent.data, vec![1, 2, 3]);

        let reply = Packet {
            hook_id: 9,
            end_hook: true,
            path: vec![PARENT],
            procedure_id: PROCEDURE,
            data: vec![4, 5, 6],
        };
        client.write_all(&reply.serialize().unwrap()).unwrap();
        bridge.update(&mut endpoint);

        let mut received = Vec::new();
        endpoint.take_inbound_clear(PARENT, |packet| received.push(packet.clone()));

        assert_eq!(received.len(), 1);
        assert_eq!(received[0].hook_id, 9);
        assert_eq!(received[0].path, vec![PARENT]);
        assert_eq!(received[0].data, vec![4, 5, 6]);
    }

    #[test]
    fn frame_length_waits_for_complete_packet() {
        let frame = downward_packet(3).serialize().unwrap();

        assert_eq!(next_frame_len(&frame[..frame.len() - 1]), None);
        assert_eq!(next_frame_len(&frame), Some(frame.len()));
    }
}
