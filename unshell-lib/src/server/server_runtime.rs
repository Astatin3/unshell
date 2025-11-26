use std::{
    net::TcpListener,
    sync::{Arc, Mutex},
    thread::{self, JoinHandle},
};

use crate::{config::RuntimeConfig, module::Manager, *};

pub struct ListenerRuntime {
    config: &'static RuntimeConfig,
    thread_handle: Option<JoinHandle<()>>,
    // streams: Arc<Mutex<Vec<TcpStream>>>,
    // manager: Option<Arc<Mutex<Manager>>>,
}

impl ListenerRuntime {
    pub fn new(config: &'static RuntimeConfig) -> Result<Self, ModuleError> {
        Ok(Self {
            config,
            thread_handle: None,
            // streams: Arc::new(Mutex::new(Vec::new())),
            // manager: None,
        })
    }

    // pub fn send(&mut self, announcement: &Announcement) -> Result<(), std::io::Error> {
    //     let bytes = announcement.encode();

    //     let mut streams = self.streams.lock().unwrap();

    //     for stream in streams.iter_mut() {
    //         stream.write_all(&u32::to_be_bytes(bytes.len() as u32))?;
    //         stream.write_all(&bytes)?;
    //         stream.flush()?;
    //     }

    //     debug!("Announcement {:?} sent", announcement);

    //     Ok(())
    // }

    // pub fn recv(&mut self) -> Result<Announcement, ModuleError> {
    //     let stream = &mut self.streams.lock().unwrap()[0];

    //     let mut size_buf = [0u8; 4];
    //     stream.read_exact(&mut size_buf).unwrap();
    //     let size = u32::from_be_bytes(size_buf);

    //     let mut buf = vec![0u8; size as usize];

    //     stream.read_exact(&mut buf).unwrap();

    //     if let Some(announcement) = Announcement::decode(&buf) {
    //         Ok(announcement)
    //     } else {
    //         Err(ModuleError::Error("Failed to decode announcement".into()))
    //     }
    // }
}

impl ModuleRuntime for ListenerRuntime {
    fn init(&mut self, manager: Arc<Mutex<Manager>>) -> Result<(), ModuleError> {
        let host = match self.config.config.get("host") {
            Some(host) => host,
            None => {
                return Err(ModuleError::Error(
                    "Could not find HOST in Server Runtime".into(),
                ));
            }
        };

        let listener = TcpListener::bind(host).unwrap();
        // let streams = Arc::new(Mutex::new(Vec::new()));

        // let streams_clone = streams.clone();

        let thread_handle = thread::spawn(move || {
            // let streams = streams_clone.clone();
            for stream in listener.incoming() {
                let stream = stream.unwrap();
                debug!("New connection from {}", stream.peer_addr().unwrap());

                let stream = crate::network::TcpStream::new(stream);

                manager.lock().unwrap().add_connection(Box::new(stream));

                // streams.lock().unwrap().push(stream);
            }
        });

        self.thread_handle = Some(thread_handle);

        Ok(())
    }

    fn is_running(&self) -> bool {
        true
    }

    fn kill(self: Box<Self>) {
        // if let Some(thread)
        // if !self.thread_handle.is_finished() {
        //     // self.join_signal.store(true, Ordering::Relaxed);
        //     let _ = self.thread_handle.join();
        // }
        // // drop(self);
    }
}
