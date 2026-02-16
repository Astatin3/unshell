// mod connection;
mod tcp_stream;

pub use tcp_stream::TcpStream;

use unshell_lib::ModuleError;

/// This is the data transmission type
pub trait Stream<T>: Send + Sync {
    // fn get_info(&self) -> String;
    fn is_alive(&self) -> bool;

    fn has_recv(&self) -> bool;

    /// Possibly blocking stream read function
    fn read(&mut self) -> Vec<T>;

    /// Non-blocking read function
    fn try_read(&mut self) -> Vec<T> {
        if self.has_recv() {
            self.read()
        } else {
            Vec::new()
        }
    }

    fn write(&mut self, data: T) -> Result<(), ModuleError>;

    fn try_clone(&self) -> Result<Box<dyn Stream<T> + Send + Sync>, ModuleError>;
}
