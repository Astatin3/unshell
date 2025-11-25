mod connection;

pub use connection::Connection;

use crate::ModuleError;

/// This is the data transmission type
pub trait Stream<T>: Send + Sync {
    // fn get_info(&self) -> String;
    fn is_alive(&self) -> bool;

    fn len(&self) -> usize;
    fn read(&self) -> Vec<T>;

    fn write(&mut self, data: T) -> Result<(), ModuleError>;

    fn try_clone(&self) -> Result<Box<dyn Stream<T> + Send + Sync>, ModuleError>;
}
