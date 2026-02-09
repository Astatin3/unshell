mod obs_junk_asm;
mod obs_xor;
mod sym_aes_strings;

pub use obs_junk_asm::junk_asm;
pub use obs_xor::xor;
pub use sym_aes_strings::*;

use crate::crypt::{BACKUP_ENV_KEY, ENV_KEY_NAME};

fn get_encryption_key() -> String {
    std::env::var(ENV_KEY_NAME).unwrap_or({
        println!("Using default encryption key!");
        BACKUP_ENV_KEY.to_owned()
    })
}

// pub fn
