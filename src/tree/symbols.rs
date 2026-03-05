use crate::obfuscate::sym;

pub const LOGGER: &'static str = sym!("Logger");

pub const TYPE_TREE: &'static str = sym!("Tree");
pub const TYPE_QUEUE: &'static str = sym!("Queue");

pub const CMD_GET: &'static str = sym!("Get");
pub const CMD_POLL: &'static str = sym!("Poll");
pub const CMD_GET_LENGTH: &'static str = sym!("GetLength");
pub const CMD_GET_CHILDREN: &'static str = sym!("GetChildren");

pub const ERR_UNSUPPORTED_METHOD: &'static str = sym!("UnsupportedMethod");
pub const ERR_INVALID_COMMAND: &'static str = sym!("InvalidCommand");
pub const ERR_INVALID_CHILD: &'static str = sym!("InvalidChild");
