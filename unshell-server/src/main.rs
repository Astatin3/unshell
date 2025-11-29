use axum;
use tokio::net::TcpListener;
use unshell_lib::info;

use unshell_server::start_api;

#[tokio::main]
async fn main() {
    unshell_lib::logger::PrettyLogger::init();

    start_api("localhost:3000").await;
}
