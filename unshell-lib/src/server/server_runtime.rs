use std::{
    io::Write,
    net::{TcpListener, TcpStream},
    sync::{Arc, Mutex},
    thread::{self, JoinHandle},
};

use crate::ModuleRuntime;

use crate::Announcement;

pub struct ListenerRuntime {
    thread_handle: JoinHandle<()>,
    // listener: TcpListener,
    streams: Arc<Mutex<Vec<TcpStream>>>,
    // reader: BufReader<TcpListener>,
    // writer: BufWriter<TcpListener>,
}

impl ListenerRuntime {
    pub fn new() -> ListenerRuntime {
        info!("Starting listener runtime on 127.0.0.1:1234");
        let listener = TcpListener::bind("127.0.0.1:1234").unwrap();
        let streams = Arc::new(Mutex::new(Vec::new()));

        let streams_clone = streams.clone();

        let thread_handle = thread::spawn(move || {
            let streams = streams_clone.clone();
            for stream in listener.incoming() {
                let stream = stream.unwrap();
                println!("New connection from {}", stream.peer_addr().unwrap());
                streams.lock().unwrap().push(stream);
            }
        });
        Self {
            thread_handle,
            streams,
        }
    }

    pub fn send(&mut self, announcement: &Announcement) -> Result<(), std::io::Error> {
        let bytes = announcement.encode();

        let mut streams = self.streams.lock().unwrap();

        for stream in streams.iter_mut() {
            stream.write_all(&u32::to_be_bytes(bytes.len() as u32))?;
            stream.write_all(&bytes)?;
            stream.flush()?;
        }

        println!("Announcement {:?} sent", announcement);

        Ok(())

        // self.stream
        //     .write_all(&u32::to_be_bytes(bytes.len() as u32))?;
        // self.stream.write_all(&bytes)?;
        // self.stream.flush()?;
    }
}

impl ModuleRuntime for ListenerRuntime {
    // fn init(&mut self) {}

    fn is_running(&self) -> bool {
        true
    }

    fn kill(self: Box<Self>) {
        if !self.thread_handle.is_finished() {
            let _ = self.thread_handle.join();
        }
        // drop(self);
    }
}
