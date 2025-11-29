use axum::{
    Extension, Router,
    extract::Path,
    middleware,
    response::IntoResponse,
    routing::{get, post},
};
use tokio::net::TcpListener;
use unshell_lib::info;

use crate::api::{auth, structs::CurrentUser};

pub async fn start_api(address: &str) {
    let listener = TcpListener::bind(address)
        .await
        .expect("Unable to start listener");

    info!("Listening on {}", listener.local_addr().unwrap());

    let app = Router::new().route("/auth", post(auth::sign_in)).route(
        "/api/{*path}",
        get(protected).layer(middleware::from_fn(auth::authorize)),
    );

    axum::serve(listener, app)
        .await
        .expect("Error serving application");
}

pub async fn protected(
    Path(path): Path<String>,
    Extension(currentUser): Extension<CurrentUser>,
) -> impl IntoResponse {
    info!("{}", path);
    // Json(UserResponse {
    //     email: currentUser.email,
    //     first_name: currentUser.first_name,
    //     last_name: currentUser.last_name,
    // })
    "Test"
}
