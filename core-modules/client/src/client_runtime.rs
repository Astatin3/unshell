use std::{
    net,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
    thread::{self, JoinHandle},
    time::Duration,
};

use unshell_lib::{
    config::RuntimeConfig,
    module::Manager,
    network::{Stream, TcpStream},
    *,
};
// use unshell_modules::{Manager, ModuleRuntime};

use unshell_lib::ModuleRuntime;

pub struct ClientRuntime {
    config: &'static RuntimeConfig,
    thread_handle: Option<JoinHandle<()>>,
    join_signal: Arc<AtomicBool>,
}

impl ClientRuntime {
    pub fn new(config: &'static RuntimeConfig) -> Result<ClientRuntime, ModuleError> {
        let join_signal = Arc::new(AtomicBool::new(false));

        Ok(Self {
            config,
            thread_handle: None,
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
    fn is_running(&self) -> bool {
        self.thread_handle.as_ref().is_none_or(|h| h.is_finished())
    }

    fn kill(self: Box<Self>) {
        if !self.is_running() {
            self.join_signal.store(true, Ordering::Relaxed);
            if let Some(handle) = self.thread_handle {
                let _ = handle.join();
            }
        }
    }

    fn init(&mut self, manager: Arc<Mutex<Manager>>) -> Result<(), ModuleError> {
        let host = match self.config.config.get("host") {
            Some(host) => host,
            None => {
                return Err(ModuleError::Error(
                    "Could not find HOST in Client Runtime".into(),
                ));
            }
        };

        let retry = match self.config.config.get("retry") {
            Some(retry) => Duration::from_millis(retry.parse::<u64>().unwrap()),
            None => {
                return Err(ModuleError::Error(
                    "Could not find RETRY in Client Runtime".into(),
                ));
            }
        };

        // let join_clone = self.join_signal.clone();

        thread::spawn(move || {
            debug!("Connecting to server...");

            loop {
                let stream = match net::TcpStream::connect(host) {
                    Ok(stream) => stream,
                    Err(e) => {
                        error!("Failed to connect to server: {}", e);
                        thread::sleep(retry);
                        continue;
                    }
                };
                info!("Connected to {}", host);

                thread::sleep(Duration::from_millis(100));
                // Duration::from_millis(100);

                let stream = TcpStream::new(stream);
                let stream_clone = stream.try_clone().unwrap();

                manager.lock().unwrap().add_connection(stream_clone);

                // while !join_clone.load(Ordering::Relaxed) {

                // }

                while stream.is_alive() {
                    thread::sleep(Duration::from_millis(100));
                }

                debug!("Disconnected from 1234 {}", host);

                thread::sleep(retry);
            }
        });

        Ok(())
    }
}
