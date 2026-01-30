mod api;
mod auth;
// mod config;
pub mod logger;
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
pub static SERVER_CONFIG: unshell_manager::PayloadConfig = unshell_manager::PayloadConfig {
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

static JWT_SECRET: String = {
    if let Ok(env_secret) = std::env::var("JWT_SECRET") {
        env_secret
    } else {
        println!(
            r#"
##############
# WARNING: You are using the default JWT secret, used for creating user sessions
# With this default key, anyone can login as any user.
##############"#
        );
        "DEFAULT_SECRET".to_string()
    }
};

// std::env::var("JWT_SECRET").unwrap_or(|| -> String {
//     return "TEST".to_string();
// }());

#[dynamic]
static JWT_ENCODING_KEY: EncodingKey = EncodingKey::from_secret(JWT_SECRET.as_bytes());
#[dynamic]
static JWT_DECODING_KEY: DecodingKey = DecodingKey::from_secret(JWT_SECRET.as_bytes());
