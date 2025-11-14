use std::{
    io::Read,
    net::TcpStream,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
    thread::{self, JoinHandle},
};

use crate::{config::RuntimeConfig, *};
// use unshell_modules::{Manager, ModuleRuntime};

use crate::{Announcement, ModuleRuntime};

pub struct ClientRuntime {
    thread_handle: JoinHandle<()>,
    join_signal: Arc<AtomicBool>,
}

impl ClientRuntime {
    pub fn new(config: &'static RuntimeConfig) -> Result<ClientRuntime, ModuleError> {
        let join_signal = Arc::new(AtomicBool::new(false));
        let join_clone = join_signal.clone();

        let host = match config.config.get("host") {
            Some(host) => host,
            None => {
                return Err(ModuleError::Error(
                    "Could not find HOST in Client Runtime".into(),
                ));
            }
        };

        Ok(Self {
            thread_handle: thread::spawn(move || {
                debug!("Connecting to server...");
                let mut stream = match TcpStream::connect(host) {
                    Ok(stream) => stream,
                    Err(e) => {
                        error!("Failed to connect to server: {}", e);
                        return;
                    }
                };
                info!("Connected");

                while !join_clone.load(Ordering::Relaxed) {
                    let mut size_buf = [0u8; 4];
                    stream.read_exact(&mut size_buf).unwrap();
                    let size = u32::from_be_bytes(size_buf);

                    let mut buf = vec![0u8; size as usize];

                    stream.read_exact(&mut buf).unwrap();

                    let a = Announcement::decode(&buf).unwrap();

                    match a {
                        Announcement::TestAnnouncement(s) => {
                            println!("Received test announcement: {}", s)
                        }
                        _ => {}
                    }
                }
            }),
            join_signal,
        })
    }

    // pub fn send(&mut self, announcement: &Announcement) -> Result<(), ModuleError> {
    //     let bytes = announcement.encode();

    //     let mut streams = self.stream.lock().unwrap();

    //     for stream in streams.iter_mut() {
    //         stream.write_all(&u32::to_be_bytes(bytes.len() as u32))?;
    //         stream.write_all(&bytes)?;
    //         stream.flush()?;
    //     }

    //     println!("Announcement {:?} sent", announcement);

    //     Ok(())
    // }
}

impl ModuleRuntime for ClientRuntime {
    // fn init(&mut self) {}

    fn is_running(&self) -> bool {
        // println!("Checking if running");
        !self.thread_handle.is_finished()
    }

    fn kill(self: Box<Self>) {
        if !self.thread_handle.is_finished() {
            self.join_signal.store(true, Ordering::Relaxed);
            let _ = self.thread_handle.join();
        }
        // drop(self);
    }
}
