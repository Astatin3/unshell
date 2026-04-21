//! # REPL Command Parser
//!
//! Parses lines typed in the operator REPL into structured `Command` values.
//!
//! ## Supported commands
//!
//! | Command | Description |
//! |---|---|
//! | `list` | List all connected nodes |
//! | `use <path>` | Set the current working path |
//! | `ls [path]` | List procedures at `path` (or current path) |
//! | `call <path> [data]` | Call a procedure at `path` |
//! | `read <path>` | Read a value at `path` |
//! | `write <path> <data>` | Write a value to `path` |
//! | `background` | Background the current session |
//! | `sessions` | List backgrounded sessions |
//! | `exit` | Disconnect and quit |
//! | `help` | Print this help |

/// A parsed REPL command.
#[derive(Debug, Clone, PartialEq)]
pub enum Command {
    /// `list` — list all connected nodes via `/router/nodes`.
    List,
    /// `use <path>` — set the current working path.
    Use(String),
    /// `ls [path]` — `GetProcedures` at the given or current path.
    Ls(Option<String>),
    /// `call <path> [data]` — `CallProcedure` at `path` with optional `data`.
    Call { path: String, data: Option<String> },
    /// `read <path>` — `Read` at `path`.
    Read(String),
    /// `write <path> <data>` — `Write` at `path` with `data`.
    Write { path: String, data: String },
    /// `background` — push current session to background list.
    Background,
    /// `sessions` — list backgrounded sessions.
    Sessions,
    /// `exit` — disconnect and quit.
    Exit,
    /// `help` — print command help.
    Help,
}

/// Parse a line of input into a `Command`.
///
/// Returns `None` if the line is empty or a comment (`#`).
/// Returns `Err` if the line cannot be parsed as a valid command.
///
/// # Example
///
/// ```rust
/// use ush_cli::commands::{parse, Command};
///
/// assert_eq!(parse("list").unwrap(), Some(Command::List));
/// assert_eq!(parse("use /agents/abc123").unwrap(), Some(Command::Use("/agents/abc123".into())));
/// assert_eq!(parse("").unwrap(), None);
/// assert_eq!(parse("  # comment").unwrap(), None);
/// ```
///
/// # Errors
///
/// Returns an error string if the command name is unrecognised or the
/// arguments are malformed.
pub fn parse(line: &str) -> Result<Option<Command>, String> {
    let trimmed = line.trim();

    // Empty lines and comments
    if trimmed.is_empty() || trimmed.starts_with('#') {
        return Ok(None);
    }

    let mut parts = trimmed.splitn(3, ' ');
    let cmd = parts.next().unwrap_or("");
    let arg1 = parts.next().map(str::trim);
    let arg2 = parts.next().map(str::trim);

    match cmd {
        "list" => Ok(Some(Command::List)),
        "use" => {
            let path = arg1.ok_or("usage: use <path>")?;
            Ok(Some(Command::Use(path.to_owned())))
        }
        "ls" => Ok(Some(Command::Ls(arg1.map(str::to_owned)))),
        "call" => {
            let path = arg1.ok_or("usage: call <path> [data]")?;
            Ok(Some(Command::Call {
                path: path.to_owned(),
                data: arg2.map(str::to_owned),
            }))
        }
        "read" => {
            let path = arg1.ok_or("usage: read <path>")?;
            Ok(Some(Command::Read(path.to_owned())))
        }
        "write" => {
            let path = arg1.ok_or("usage: write <path> <data>")?;
            let data = arg2.ok_or("usage: write <path> <data>")?;
            Ok(Some(Command::Write {
                path: path.to_owned(),
                data: data.to_owned(),
            }))
        }
        "background" | "bg" => Ok(Some(Command::Background)),
        "sessions" => Ok(Some(Command::Sessions)),
        "exit" | "quit" | "q" => Ok(Some(Command::Exit)),
        "help" | "?" => Ok(Some(Command::Help)),
        other => Err(format!("unknown command: {other}. Type 'help' for a list.")),
    }
}

/// Print the help text for all available commands.
pub fn print_help() {
    println!("Available commands:");
    println!("  list                   List all connected nodes");
    println!("  use <path>             Set working path (e.g., use agents/abc123)");
    println!("  ls [path]              List available procedures");
    println!("  call <path> [data]     Call a procedure");
    println!("  read <path>            Read a value");
    println!("  write <path> <data>    Write a value");
    println!("  background             Background current session");
    println!("  sessions               List backgrounded sessions");
    println!("  exit                   Disconnect and quit");
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_empty() {
        assert_eq!(parse("").unwrap(), None);
        assert_eq!(parse("   ").unwrap(), None);
        assert_eq!(parse("# comment").unwrap(), None);
    }

    #[test]
    fn parse_list() {
        assert_eq!(parse("list").unwrap(), Some(Command::List));
    }

    #[test]
    fn parse_use() {
        assert_eq!(
            parse("use /agents/abc123").unwrap(),
            Some(Command::Use("/agents/abc123".into()))
        );
    }

    #[test]
    fn parse_ls_no_arg() {
        assert_eq!(parse("ls").unwrap(), Some(Command::Ls(None)));
    }

    #[test]
    fn parse_ls_with_arg() {
        assert_eq!(
            parse("ls shell").unwrap(),
            Some(Command::Ls(Some("shell".into())))
        );
    }

    #[test]
    fn parse_call_with_data() {
        assert_eq!(
            parse("call shell/exec ls -la").unwrap(),
            Some(Command::Call {
                path: "shell/exec".into(),
                data: Some("ls -la".into()),
            })
        );
    }

    #[test]
    fn parse_exit_aliases() {
        assert_eq!(parse("exit").unwrap(), Some(Command::Exit));
        assert_eq!(parse("quit").unwrap(), Some(Command::Exit));
        assert_eq!(parse("q").unwrap(), Some(Command::Exit));
    }

    #[test]
    fn parse_unknown_command() {
        assert!(parse("foobar").is_err());
    }
}
