use chrono::Duration;
use jsonwebtoken::{DecodingKey, EncodingKey};
use static_init::dynamic;
extern crate unshell_lib;

pub mod app;
mod auth;
mod structs;

pub use structs::CurrentUser;

static EXPIRE_DURATION: Duration = Duration::hours(12);

#[dynamic]
static JWT_SECRET: String = std::env::var("JWT_SECRET").expect("JWT_SECRET must be set");

#[dynamic]
static JWT_ENCODING_KEY: EncodingKey = EncodingKey::from_secret(JWT_SECRET.as_bytes());
#[dynamic]
static JWT_DECODING_KEY: DecodingKey = DecodingKey::from_secret(JWT_SECRET.as_bytes());
