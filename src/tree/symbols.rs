use crate::obfuscate::symbol;

pub const LOGGER: &'static str = symbol!("Logger");

pub const TYPE_TREE: &'static str = symbol!("Tree");
pub const TYPE_QUEUE: &'static str = symbol!("Queue");

pub const CMD_GET: &'static str = symbol!("Get");
pub const CMD_POLL: &'static str = symbol!("Poll");
pub const CMD_GET_LENGTH: &'static str = symbol!("GetLength");
pub const CMD_GET_CHILDREN: &'static str = symbol!("GetChildren");

pub const ERR_UNSUPPORTED_METHOD: &'static str = symbol!("UnsupportedMethod");
pub const ERR_INVALID_COMMAND: &'static str = symbol!("InvalidCommand");
pub const ERR_INVALID_CHILD: &'static str = symbol!("InvalidChild");
pub const ERR_INVALID_TARGET: &'static str = symbol!("InvalidTarget");
pub const ERR_CHILD_NOT_FOUND: &'static str = symbol!("ChildNotFound");
pub const ERR_INVALID_PATH: &'static str = symbol!("InvalidPath");
