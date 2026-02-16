use std::{
    io::{Read, Write},
    net,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
};

use unshell_lib::{Announcement, ModuleError, Result, debug};

use crate::network::Stream;

pub struct TcpStream(Arc<AtomicBool>, net::TcpStream);

impl TcpStream {
    pub fn new(stream: net::TcpStream) -> Self {
        stream.set_nonblocking(true).unwrap();
        Self(Arc::new(AtomicBool::new(true)), stream)
    }

    // Call this when the stream ends
    fn disconnected(&mut self) {
        self.0.store(false, Ordering::Relaxed);
    }
}

impl Stream<Announcement> for TcpStream {
    fn is_alive(&self) -> bool {
        // if self.1.take_error().unwrap_or(None).is_some() {
        //     // self.1.pe
        //     warn!("Disconnected #################");
        //     return true;
        // } else {
        //     return false;
        // }

        // let mut buf = [0u8; 1];
        // match self.1.peek(&mut buf) {
        //     Ok(n) => n == 1,
        //     Err(_) => false,
        // }

        let mut buf = [0u8; 1];
        match self.1.peek(&mut buf) {
            Ok(0) => false, // Connection closed (EOF)
            Ok(_) => true,  // Data available or connection alive
            Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => true, // No data but alive
            Err(_) => false, // Connection error
        }

        // true

        // self.0.load(Ordering::Relaxed)
    }

    fn has_recv(&self) -> bool {
        let mut buf = [0u8; 1];
        match self.1.peek(&mut buf) {
            Ok(n) if n > 0 => true, // Data is available
            Ok(_) => false,         // EOF (connection closed)
            Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => false, // No data
            Err(_) => false,
        }
        // false
    }

    fn read(&mut self) -> Vec<Announcement> {
        let mut ret = Vec::new();

        while self.has_recv() {
            let mut size_buf = [0u8; 4];
            match self.1.read_exact(&mut size_buf) {
                Ok(()) => {}
                Err(_) => {
                    self.disconnected();
                    break;
                }
            };
            let size = u32::from_be_bytes(size_buf);

            let mut buf = vec![0u8; size as usize];

            match self.1.read_exact(&mut buf) {
                Ok(()) => {}
                Err(_) => {
                    self.disconnected();
                }
            }

            if let Some(a) = Announcement::decode(&buf) {
                ret.push(a);
            } else {
                debug!("Malformed data");
            }
        }

        ret
    }

    fn write(&mut self, announcement: Announcement) -> Result<()> {
        let bytes = announcement.encode();

        // Write length of bytes
        self.1
            .write_all(&u32::to_be_bytes(bytes.len() as u32))
            .map_err(|e| ModuleError::Error(e.to_string().into()))?;
        // Write data
        self.1
            .write_all(&bytes)
            .map_err(|e| ModuleError::Error(e.to_string().into()))?;
        // Flush data
        self.1
            .flush()
            .map_err(|e| ModuleError::Error(e.to_string().into()))?;

        Ok(())
    }

    fn try_clone(&self) -> Result<Box<dyn Stream<Announcement> + Send + Sync>> {
        Ok(Box::new(Self(
            self.0.clone(),
            self.1
                .try_clone()
                .map_err(|e| ModuleError::Error(e.to_string().into()))?,
        )))
    }
}
