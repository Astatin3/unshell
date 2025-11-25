use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};

use crate::{Announcement, ModuleError, network::Stream};

use crossbeam_channel::{Receiver, Sender};

pub struct Connection {
    tx: Sender<Announcement>,
    rx: Receiver<Announcement>,
    is_alive: Arc<AtomicBool>,
}

impl Connection {
    pub fn new() -> (Connection, Connection) {
        let (tx_mgr, rx) = crossbeam_channel::unbounded();
        let (tx, rx_mgr) = crossbeam_channel::unbounded();
        let alive = Arc::new(AtomicBool::new(false));

        (
            Self {
                tx: tx_mgr,
                rx: rx_mgr,
                is_alive: alive.clone(),
            },
            Self {
                tx,
                rx,
                is_alive: alive,
            },
        )
    }
}

impl Stream<Announcement> for Connection {
    fn is_alive(&self) -> bool {
        self.is_alive.load(Ordering::Relaxed)
    }

    fn len(&self) -> usize {
        self.rx.len()
    }

    fn read(&mut self) -> Option<Announcement> {
        match self.rx.is_empty() {
            true => None,
            false => self.rx.recv().ok(),
        }
    }

    fn write(&mut self, data: Announcement) -> Result<(), crate::ModuleError> {
        self.tx
            .send(data)
            .map_err(|_| ModuleError::Error("Failed to send".into()))?;

        Ok(())
    }

    fn try_clone(&self) -> Result<Box<dyn Stream<Announcement> + Send + Sync>, crate::ModuleError> {
        Ok(Box::new(Self {
            tx: self.tx.clone(),
            rx: self.rx.clone(),
            is_alive: self.is_alive.clone(),
        }))
    }
}
