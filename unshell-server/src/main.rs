use axum;
use tokio::net::TcpListener;
use unshell_lib::info;

use unshell_server::app;

#[tokio::main]
async fn main() {
    unshell_lib::logger::PrettyLogger::init();

    let listener = TcpListener::bind("127.0.0.1:3000")
        .await
        .expect("Unable to start listener");

    info!("Listening on {}", listener.local_addr().unwrap());

    let app = app::app().await;

    axum::serve(listener, app)
        .await
        .expect("Error serving application");
}
