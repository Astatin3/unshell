use axum::{
    Extension, Json, Router,
    extract::{Path, State},
    middleware,
    response::IntoResponse,
    routing::{get, post},
};
use tokio::net::TcpListener;
use unshell_lib::info;

use crate::{
    api::{auth, structs::CurrentUser},
    database::Database,
};

pub async fn start_api(address: &str, database: Database) {
    let listener = TcpListener::bind(address)
        .await
        .expect("Unable to start listener");

    info!("Listening on {}", listener.local_addr().unwrap());

    let app = Router::new()
        .route("/api/auth", post(auth::sign_in))
        .route(
            "/api/{*path}",
            get(get_data).layer(middleware::from_fn(auth::authorize)),
        )
        .route(
            "/api/{*path}",
            post(post_data).layer(middleware::from_fn(auth::authorize)),
        )
        .with_state(database);

    axum::serve(listener, app)
        .await
        .expect("Error serving application");
}

pub async fn get_data(
    State(database): State<Database>,
    Path(path): Path<String>,
    Extension(_): Extension<CurrentUser>,
) -> impl IntoResponse {
    let result = database.get_value(&path);

    Json(serde_json::to_value(result).unwrap())
}

pub async fn post_data(
    State(database): State<Database>,
    Path(path): Path<String>,
    Extension(_): Extension<CurrentUser>,
    body: String,
) -> impl IntoResponse {
    let result = database.put_value(&path, &body);

    Json(serde_json::to_value(result).unwrap())
}

// impl IntoResponse for Option<StrW> {
//     // impl IntoResponse for Option<String> {
//     fn into_response(self) -> axum::response::Response {
//         todo!()
//     }
// }
