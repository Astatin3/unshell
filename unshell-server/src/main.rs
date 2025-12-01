use unshell_server::{database::Database, start_api};

use clap::Parser;
use static_init::dynamic;

#[dynamic]
static DEFAULT_HOST: String = "localhost".to_string();
#[dynamic]
static DATABASE_NAME: String = "database".to_string();

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
}

#[tokio::main]
async fn main() {
    let args = Args::parse();

    unshell_lib::logger::PrettyLogger::init();

    let database = Database::new(args.database_name);

    start_api(&format!("{}:{}", args.host, args.port), database).await;
}
