//! # ush-cli — UnShell Operator REPL
//!
//! The operator CLI connects to the router as a first-class node and provides
//! an interactive shell for issuing commands to connected payload nodes.
//!
//! ## Usage
//!
//! ```text
//! ush-cli --router 127.0.0.1:9000
//! ```
//!
//! ## REPL commands
//!
//! ```text
//! unshell> list                            # list all connected nodes
//! unshell> use agents/abc123               # set working path prefix
//! unshell [agents/abc123]> ls              # GetProcedures at current path
//! unshell [agents/abc123]> call shell/exec "ls -la"
//! unshell [agents/abc123]> read files/passwd
//! unshell [agents/abc123]> background      # detach, keep in session list
//! unshell> sessions                        # list background sessions
//! unshell> exit                            # disconnect and quit
//! ```

mod commands;
mod repl;
mod session;

fn main() {
    // TODO: parse --router argument
    let router_addr = "127.0.0.1:9000";
    repl::run(router_addr).expect("repl failed");
}
