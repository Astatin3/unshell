const ENV_KEY_NAME: &str = "OBFUSCATION_KEY";
const BACKUP_ENV_KEY: &str = "OBFUSCATION_KEY_DO_NOT_USE";

pub fn get_encryption_key() -> String {
    if let Ok(key) = std::env::var(ENV_KEY_NAME) {
        key
    } else {
        println!("Using default encryption key!");
        BACKUP_ENV_KEY.to_owned()
    }
}
