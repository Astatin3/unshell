use std::{
    io,
    net::{Ipv4Addr, TcpListener, ToSocketAddrs},
};

use unshell::protocol::{Endpoint, Leaf};

use crate::transport::TcpBridge;

/// TCP server-side transport leaf for one downstream endpoint.
///
/// The protocol endpoint is intentionally leaf-owned by the caller, so this type
/// only bridges bytes: accepted TCP frames are deserialized into inbound packets,
/// and outbound packets queued for `child_endpoint_id` are serialized back onto the
/// same stream. Use this on the authority/parent side of a two-endpoint link.
#[derive(Debug)]
pub struct TCPServerLeaf {
    listener: TcpListener,
    bridge: TcpBridge,
}

impl TCPServerLeaf {
    /// Binds a nonblocking TCP listener for a child endpoint connection.
    ///
    /// `child_endpoint_id` must match the adjacent endpoint segment used in packet
    /// paths. The server registers that endpoint as downstream so inbound bytes from
    /// the child are treated as upward traffic by [`Endpoint::add_inbound_from`].
    pub fn new<A>(listen_addr: A, child_endpoint_id: u32) -> io::Result<Self>
    where
        A: ToSocketAddrs,
    {
        let listener = TcpListener::bind(listen_addr)?;
        listener.set_nonblocking(true)?;

        Ok(Self {
            listener,
            bridge: TcpBridge::new(child_endpoint_id, false),
        })
    }

    /// Binds a nonblocking IPv4 listener for minimized fixed-address endpoints.
    ///
    /// This avoids making tiny binaries instantiate the fully generic public
    /// constructor when they already know the concrete IPv4 address and port.
    pub fn bind_ipv4(addr: Ipv4Addr, port: u16, child_endpoint_id: u32) -> io::Result<Self> {
        let listener = TcpListener::bind((addr, port))?;
        listener.set_nonblocking(true)?;

        Ok(Self {
            listener,
            bridge: TcpBridge::new(child_endpoint_id, false),
        })
    }
}

impl Leaf for TCPServerLeaf {
    fn get_id(&self) -> u32 {
        crate::IDENTIFIER_SERVER_HASH
    }

    fn update(&mut self, endpoint: &mut Endpoint) {
        self.bridge.register(endpoint);
        self.accept_connection();
        self.bridge.update(endpoint);
    }
}

impl TCPServerLeaf {
    /// Accepts at most one active stream without blocking the endpoint loop.
    ///
    /// A second accepted stream would make packet ownership ambiguous for the same
    /// `child_endpoint_id`, so the minimal bridge keeps the first live connection and
    /// waits for it to disconnect before accepting another.
    fn accept_connection(&mut self) {
        if self.bridge.is_connected() {
            return;
        }

        if let Ok((stream, _)) = self.listener.accept() {
            let _ = self.bridge.set_stream(stream);
        }
    }
}
