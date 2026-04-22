//! # Unshell Tree Protocol Testbed
//!
//! This is a testbed implementation of a tree-based routing protocol for unshell.
//! It supports serving and connecting to tree endpoints, with leaves for RemoteShell
//! (command execution) and TTY (PTY streaming).
//!
//! # Commands
//!
//! - `serve [addr]` - Start a server
//! - `connect [addr]` - Connect to a server and run CLI
//! - `run <command>` - Run a single command locally
//! - (default) - Run interactive CLI with local tree
//!
//! # Example
//!
//! ```bash
//! # Start server
//! $ ush-treetest serve 0.0.0.0:8080
//!
//! # Connect from another terminal
//! $ ush-treetest connect localhost:8080
//! ```

mod cli;
mod client;
mod leaves;
mod protocol;
mod server;
mod tree;

use crate::cli::{Cli, parse_and_execute};
use std::io::{self, Write};
use clap::{Parser, Subcommand};

/// CLI argument parser.
///
/// # Example
/// ```
/// let args = Args::parse();
/// match args.command {
///     Some(Command::Serve { addr }) => { ... }
///     Some(Command::Connect { addr }) => { ... }
///     _ => { ... }
/// }
/// ```
#[derive(Parser)]
#[command(name = "ush-treetest")]
#[command(about = "Unshell tree protocol testbed")]
struct Args {
    #[command(subcommand)]
    command: Option<Command>,

    #[arg(short, long)]
    addr: Option<String>,
}

/// Subcommands for the CLI.
///
/// # Variants
/// - `Serve` - Start a server
/// - `Connect` - Connect to a server
/// - `Run` - Run a single command locally
/// - `Cli` - Run interactive CLI (default)
#[derive(Subcommand)]
enum Command {
    /// Start a server
    Serve {
        /// Address to listen on
        #[arg(default_value = "0.0.0.0:8080")]
        addr: String,
    },
    /// Connect to a server
    Connect {
        /// Server address to connect to
        #[arg(default_value = "localhost:8080")]
        addr: String,
    },
    /// Run interactive CLI
    Cli {},
    /// Run a single command locally
    Run {
        /// Command to execute
        command: String,
    },
}

/// Main entry point.
///
/// # Example
/// ```
/// // Start server
/// $ ush-treetest serve
///
/// // Connect to server
/// $ ush-treetest connect localhost:8080
///
/// // Run locally
/// $ ush-treetest run "exec /shell echo hello"
/// ```
fn main() {
    let _ = env_logger::try_init();

    let args = Args::parse();

    match args.command {
        Some(Command::Serve { addr }) => {
            server::run_server(&addr);
        }
        Some(Command::Connect { addr }) => {
            run_client(&addr);
        }
        Some(Command::Run { command }) => {
            run_single_command(&command);
        }
        None | Some(Command::Cli {}) => {
            run_interactive();
        }
    }
}

/// Run the client with connection to a server.
///
/// # Arguments
/// * `addr` - Server address
fn run_client(addr: &str) {
    let mut cli = Cli::new();

    if let Err(e) = cli.connect(addr) {
        eprintln!("Failed to connect: {}", e);
        return;
    }

    println!("Connected to {}", addr);
    run_cli_loop(&mut cli);
}

/// Run an interactive CLI with a local tree.
fn run_interactive() {
    let mut cli = Cli::new();

    println!("Unshell Tree Protocol Testbed");
    println!("Type 'help' for commands\n");
    println!("Local tree with endpoints:");
    for leaf in cli.list_leaves() {
        println!("  {}", leaf);
    }
    println!();

    run_cli_loop(&mut cli);
}

/// Run the CLI command loop.
///
/// # Arguments
/// * `cli` - The CLI instance
fn run_cli_loop(cli: &mut Cli) {
    loop {
        print!("{}> ", cli.current_path());
        io::stdout().flush().ok();

        let mut line = String::new();
        if io::stdin().read_line(&mut line).is_err() {
            break;
        }

        let line = line.trim();

        if line.is_empty() {
            continue;
        }

        if line == "quit" || line == "exit" {
            break;
        }

        match parse_and_execute(cli, line) {
            Ok(output) => {
                if !output.is_empty() {
                    println!("{}", output);
                }
            }
            Err(e) => {
                eprintln!("Error: {}", e);
            }
        }
    }
}

/// Run a single command locally.
///
/// # Arguments
/// * `command` - The command to run
fn run_single_command(command: &str) {
    let mut cli = Cli::new();

    match parse_and_execute(&mut cli, command) {
        Ok(output) => {
            if !output.is_empty() {
                println!("{}", output);
            }
        }
        Err(e) => {
            eprintln!("Error: {}", e);
            std::process::exit(1);
        }
    }
}