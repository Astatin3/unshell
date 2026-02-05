mod log;

pub struct Manager {
    // logs_tx: Sender<Record>,
    // logs_rx: Receiver<Record>,
}

impl Manager {
    pub fn new() -> Self {
        Self {}

        // let (tx, rx) = crossbeam_channel::unbounded();

        // Self {
        //     logs_tx: tx,
        //     logs_rx: rx,
        // }
    }

    // pub fn log_count(&self) -> usize {
    //     self.logs_rx.len()
    // }
}
