use std::{io, net::TcpStream, net::ToSocketAddrs};

use unshell::protocol::{Endpoint, Leaf};

use crate::transport::TcpBridge;

/// TCP client-side transport leaf for one upstream endpoint.
///
/// This is the mirror of [`crate::TCPServerLeaf`]: bytes from the connected server
/// are routed through [`Endpoint::add_inbound_from`], and packets queued for the
/// parent endpoint are serialized back onto the TCP stream.
#[derive(Debug)]
pub struct TCPClientLeaf {
    bridge: TcpBridge,
}

impl TCPClientLeaf {
    /// Connects to an upstream TCP server and registers it as the authority peer.
    ///
    /// `parent_endpoint_id` must be the adjacent parent segment in this endpoint's
    /// path. The connection is made during construction so failed startup is explicit
    /// instead of being hidden as a permanently idle leaf.
    pub fn new<A>(connect_addr: A, parent_endpoint_id: u32) -> io::Result<Self>
    where
        A: ToSocketAddrs,
    {
        let stream = TcpStream::connect(connect_addr)?;
        let mut bridge = TcpBridge::new(parent_endpoint_id, true);
        bridge.set_stream(stream)?;

        Ok(Self { bridge })
    }
}

impl Leaf for TCPClientLeaf {
    fn get_id(&self) -> u32 {
        crate::IDENTIFIER_CLIENT_HASH
    }

    fn update(&mut self, endpoint: &mut Endpoint) {
        self.bridge.register(endpoint);
        self.bridge.update(endpoint);
    }
}
