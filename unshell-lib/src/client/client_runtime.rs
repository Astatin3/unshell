use std::{
    io::Read,
    net::TcpStream,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
    thread::{self, JoinHandle},
};

// use unshell_modules::{Manager, ModuleRuntime};

use crate::{Announcement, ModuleRuntime, module::Manager};

pub struct RuntimeTest {
    thread_handle: JoinHandle<()>,
    join_signal: Arc<AtomicBool>,
}

impl RuntimeTest {
    pub fn new(_manager: Arc<Mutex<Manager>>) -> RuntimeTest {
        let join_signal = Arc::new(AtomicBool::new(false));
        let join_clone = join_signal.clone();

        Self {
            thread_handle: thread::spawn(move || {
                info!("Connecting to server...");
                let mut stream = TcpStream::connect("localhost:1234").unwrap();
                info!("Connectied");
                // let reader = BufReader::new(stream.try_clone().unwrap());
                // let mut writer = BufWriter::new(stream.try_clone().unwrap());

                // let (a, b) = crossbeam_channel::unbounded();

                // a.

                // if join_receiver.len() == 0 {
                //     join_receiver.recv().unwrap();
                // }

                while !join_clone.load(Ordering::Relaxed) {
                    let mut size_buf = [0u8; 4];
                    stream.read_exact(&mut size_buf).unwrap();
                    let size = u32::from_be_bytes(size_buf);

                    let mut buf = vec![0u8; size as usize];

                    stream.read_exact(&mut buf).unwrap();

                    let a = Announcement::decode(&buf).unwrap();

                    match a {
                        Announcement::TestAnnouncement(s) => {
                            info!("Received test announcement: {}", s)
                        }
                    }
                }
            }),
            join_signal,
        }
    }
}

impl ModuleRuntime for RuntimeTest {
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
