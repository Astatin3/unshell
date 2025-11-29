use axum::{
    body::Body,
    extract::{Json, Request},
    http::{self, Response, StatusCode},
    middleware::Next,
};
use bcrypt::{DEFAULT_COST, hash, verify};
use chrono::Utc;
use jsonwebtoken::{Header, TokenData, Validation, decode, encode};
use serde_json::{Value, json};
use unshell_lib::info;

use crate::{
    EXPIRE_DURATION, JWT_DECODING_KEY, JWT_ENCODING_KEY,
    structs::{AuthError, Cliams, CurrentUser, SignInData},
};

pub fn hash_password(password: &str) -> Result<String, bcrypt::BcryptError> {
    let hash = hash(password, DEFAULT_COST)?;
    Ok(hash)
}

pub fn encode_jwt(email: String) -> Result<(String, usize), StatusCode> {
    let now = Utc::now();
    let exp = (now + EXPIRE_DURATION).timestamp() as usize;
    let iat = now.timestamp() as usize;

    let claim = Cliams { iat, exp, email };

    let token = encode(&Header::default(), &claim, &JWT_ENCODING_KEY)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    Ok((token, exp))
}

pub fn decode_jwt(jwt: String) -> Result<TokenData<Cliams>, StatusCode> {
    decode(&jwt, &JWT_DECODING_KEY, &Validation::default())
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)
}

pub async fn authorize(mut req: Request, next: Next) -> Result<Response<Body>, AuthError> {
    let auth_header = req.headers_mut().get(http::header::AUTHORIZATION);

    let auth_header = match auth_header {
        Some(header) => header.to_str().map_err(|_| AuthError {
            message: "Empty header is not allowed".to_string(),
            status_code: StatusCode::FORBIDDEN,
        })?,
        None => {
            return Err(AuthError {
                message: "Please add the JWT token to the header".to_string(),
                status_code: StatusCode::FORBIDDEN,
            });
        }
    };
    let mut header = auth_header.split_whitespace();

    let (_, token) = (header.next(), header.next());

    let token_data = match decode_jwt(token.unwrap().to_string()) {
        Ok(data) => data,
        Err(_) => {
            return Err(AuthError {
                message: "Invalid Session".to_string(),
                status_code: StatusCode::UNAUTHORIZED,
            });
        }
    };

    // Fetch the user details from the database
    let current_user = match retrieve_user_by_email(&token_data.claims.email) {
        Some(user) => user,
        None => {
            return Err(AuthError {
                message: "Unauthorized".to_string(),
                status_code: StatusCode::UNAUTHORIZED,
            });
        }
    };

    req.extensions_mut().insert(current_user);
    Ok(next.run(req).await)
}

pub async fn sign_in(Json(user_data): Json<SignInData>) -> Result<Json<Value>, StatusCode> {
    // 1. Retrieve user from the database
    let user = match retrieve_user_by_email(&user_data.username) {
        Some(user) => user,
        None => return Err(StatusCode::UNAUTHORIZED), // User not found
    };

    // 2. Compare the password
    if !verify(&user_data.password, &user.password_hash)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
    // Handle bcrypt errors
    {
        return Err(StatusCode::UNAUTHORIZED); // Wrong password
    }

    info!(
        "Authenticated user {} for {}",
        user_data.username, EXPIRE_DURATION
    );

    // 3. Generate JWT
    let (token, experation) =
        encode_jwt(user.email).map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    // 4. Return the token
    Ok(Json(json!({
        "token": token,
        "expiration": experation,
    })))
}

fn retrieve_user_by_email(_email: &str) -> Option<CurrentUser> {
    let current_user: CurrentUser = CurrentUser {
        email: "foo".to_string(),
        first_name: "Eze".to_string(),
        last_name: "Sunday".to_string(),
        password_hash: hash_password("bar").unwrap(),
    };
    Some(current_user)
}
