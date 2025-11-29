use crate::{auth, structs::CurrentUser};
use axum::{
    Extension, Router,
    extract::Path,
    middleware,
    response::IntoResponse,
    routing::{get, post},
};
use unshell_lib::info;

pub async fn app() -> Router {
    Router::new().route("/auth", post(auth::sign_in)).route(
        "/api/{*path}",
        get(protected).layer(middleware::from_fn(auth::authorize)),
    )
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
