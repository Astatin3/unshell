mod api;
mod auth;
mod config;
pub mod logger;
mod modules;
mod server;

pub use server::Server;

use static_init::dynamic;

#[static_init::dynamic]
pub static DATABASE_TREES: Vec<&'static str> = vec!["users"];

#[static_init::dynamic]
pub static DEFAULT_HOST: String = "localhost".to_string();
#[static_init::dynamic]
pub static DATABASE_NAME: String = "database".to_string();

#[static_init::dynamic]
pub static SERVER_CONFIG: unshell_lib::config::PayloadConfig = unshell_lib::config::PayloadConfig {
    id: "Server",
    components: Vec::new(),
    runtime_config: Vec::new(),
};

// Constants for server config
pub use api::start_api;
use chrono::Duration;
use jsonwebtoken::{DecodingKey, EncodingKey};

static EXPIRE_DURATION: Duration = Duration::hours(12);

#[dynamic]
static JWT_SECRET: String = std::env::var("JWT_SECRET").expect("JWT_SECRET must be set");

#[dynamic]
static JWT_ENCODING_KEY: EncodingKey = EncodingKey::from_secret(JWT_SECRET.as_bytes());
#[dynamic]
static JWT_DECODING_KEY: DecodingKey = DecodingKey::from_secret(JWT_SECRET.as_bytes());
