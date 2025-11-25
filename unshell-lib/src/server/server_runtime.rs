use std::{
    io::{Read, Write},
    net::{TcpListener, TcpStream},
    sync::{Arc, Mutex},
    thread::{self, JoinHandle},
};

use crate::{config::RuntimeConfig, *};

pub struct ListenerRuntime {
    thread_handle: JoinHandle<()>,
    // join_signal: Arc<AtomicBool>,
    // listener: TcpListener,
    streams: Arc<Mutex<Vec<TcpStream>>>,
    // reader: BufReader<TcpListener>,
    // writer: BufWriter<TcpListener>,
}

impl ListenerRuntime {
    pub fn new(config: &'static RuntimeConfig) -> Result<Self, ModuleError> {
        // info!("Starting listener runtime on {}",);

        let host = match config.config.get("host") {
            Some(host) => host,
            None => {
                return Err(ModuleError::Error(
                    "Could not find HOST in Server Runtime".into(),
                ));
            }
        };

        let listener = TcpListener::bind(host).unwrap();
        let streams = Arc::new(Mutex::new(Vec::new()));

        let streams_clone = streams.clone();

        let thread_handle = thread::spawn(move || {
            let streams = streams_clone.clone();
            for stream in listener.incoming() {
                let stream = stream.unwrap();
                debug!("New connection from {}", stream.peer_addr().unwrap());
                streams.lock().unwrap().push(stream);
            }
        });
        Ok(Self {
            thread_handle,
            streams,
        })
    }

    pub fn send(&mut self, announcement: &Announcement) -> Result<(), std::io::Error> {
        let bytes = announcement.encode();

        let mut streams = self.streams.lock().unwrap();

        for stream in streams.iter_mut() {
            stream.write_all(&u32::to_be_bytes(bytes.len() as u32))?;
            stream.write_all(&bytes)?;
            stream.flush()?;
        }

        debug!("Announcement {:?} sent", announcement);

        Ok(())
    }

    pub fn recv(&mut self) -> Result<Announcement, ModuleError> {
        let stream = &mut self.streams.lock().unwrap()[0];

        let mut size_buf = [0u8; 4];
        stream.read_exact(&mut size_buf).unwrap();
        let size = u32::from_be_bytes(size_buf);

        let mut buf = vec![0u8; size as usize];

        stream.read_exact(&mut buf).unwrap();

        if let Some(announcement) = Announcement::decode(&buf) {
            Ok(announcement)
        } else {
            Err(ModuleError::Error("Failed to decode announcement".into()))
        }
    }
}

impl ModuleRuntime for ListenerRuntime {
    // fn init(&mut self) {}

    fn is_running(&self) -> bool {
        true
    }

    fn kill(self: Box<Self>) {
        if !self.thread_handle.is_finished() {
            // self.join_signal.store(true, Ordering::Relaxed);
            let _ = self.thread_handle.join();
        }
        // drop(self);
    }
}
