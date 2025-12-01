use unshell_server::start_api;

use clap::Parser;
use static_init::dynamic;

#[dynamic]
static DEFAULT_HOST: String = "localhost".to_string();

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
}

#[tokio::main]
async fn main() {
    let args = Args::parse();

    unshell_lib::logger::PrettyLogger::init();

    start_api(&format!("{}:{}", args.host, args.port)).await;
}
