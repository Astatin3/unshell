use axum::{
    Json,
    body::Body,
    http::{Response, StatusCode},
    response::IntoResponse,
};
use serde::{Deserialize, Serialize};
use serde_json::json;

#[derive(Deserialize)]
pub struct SignInData {
    pub username: String,
    pub password: String,
}

#[derive(Debug, Clone)]
pub struct CurrentUser {
    pub username: String,
    pub password_hash: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Cliams {
    // pub exp: u128,
    // pub iat: u128,
    //
    pub exp: usize,
    pub iat: usize,
    pub email: String,
}

pub struct AuthError {
    pub message: String,
    pub status_code: StatusCode,
}

impl IntoResponse for AuthError {
    fn into_response(self) -> Response<Body> {
        let body = Json(json!({
            "error": self.message,
        }));

        (self.status_code, body).into_response()
    }
}
