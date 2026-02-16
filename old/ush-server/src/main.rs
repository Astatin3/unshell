use std::{error::Error, path::PathBuf};

use unshell_server::{Server, start_api};

use clap::Parser;
use unshell_server::{DATABASE_NAME, DEFAULT_HOST};

/// A fictional versioning CLI
#[derive(Debug, Parser)]
#[command(name = "unshell-server")]
#[command(about = "UnShell server", long_about = None)]
pub struct Args {
    /// Host to listen on
    #[clap(long, default_value_t = DEFAULT_HOST.clone())]
    host: String,

    /// Port to listen
    #[arg(short, long, default_value_t = 3000)]
    port: usize,

    /// Name of database folder
    #[clap(short, long, default_value_t = DATABASE_NAME.clone())]
    database_name: String,

    /// Load config from path
    #[clap(short, long, value_parser)]
    pub config: Vec<PathBuf>,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let args = Args::parse();

    unshell::logger::PrettyLogger::init_output(|message| {
        if let Ok(json) = serde_json::to_string(message) {
            unshell_server::logger::Logger::log(json);
        }
    });

    let database = Server::new(args.config, args.database_name)?;

    start_api(&format!("{}:{}", args.host, args.port), database).await;

    Ok(())
}
