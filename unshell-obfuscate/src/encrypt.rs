// --- Add these imports to the top of src/lib.rs ---
use aes::{
    Aes256,
    cipher::{BlockEncryptMut, KeyIvInit},
};
use cbc::Encryptor;
use cbc::cipher::block_padding::Pkcs7;
use hex;
use sha2::{Digest, Sha256};

use crate::{BACKUP_ENV_KEY, ENV_KEY_NAME};

// type Aes256CbcEncryptor = ;

// A static, hardcoded IV. This is fine for obfuscation,
// as we're not protecting against replay attacks, just static analysis.
// This is the hex for "my_static_iv_012".
const STATIC_IV: [u8; 16] = [
    0x6d, 0x79, 0x5f, 0x73, 0x74, 0x61, 0x74, 0x69, 0x63, 0x5f, 0x69, 0x76, 0x5f, 0x30, 0x31, 0x32,
];

fn pkcs7_padded_length(input_len: usize) -> usize {
    let block_size = 16;
    ((input_len / block_size) + 1) * block_size
}

pub fn get_obfuscated_symbol_name(input: &str) -> String {
    // 1. Get the key from the environment
    // let key_str =
    //     std::env::var(ENV_KEY_NAME).expect(&format!("'{}' env var not set", ENV_KEY_NAME));

    let key_str = std::env::var(ENV_KEY_NAME).unwrap_or(BACKUP_ENV_KEY.to_owned());

    // 2. Hash the env key to get a 32-byte (256-bit) AES key
    let mut hasher = Sha256::new();
    hasher.update(key_str.as_bytes());
    let key: [u8; 32] = hasher.finalize().into();

    // 3. Encrypt the input string
    let cipher = Encryptor::<Aes256>::new(&key.into(), &STATIC_IV.into());
    let mut plaintext = input.to_string();
    let plaintext = unsafe { plaintext.as_bytes_mut() };

    let buf_len = pkcs7_padded_length(plaintext.len());
    let mut buf: Vec<u8> = vec![0; buf_len];

    buf[..plaintext.len()].copy_from_slice(plaintext);
    let ciphertext = cipher
        .encrypt_padded_mut::<Pkcs7>(&mut buf, plaintext.len())
        .expect("Could not encrypt");

    // 4. Hex-encode the result
    let hex_encoded = hex::encode(ciphertext);

    hex_encoded

    // 5. Prepend a prefix
    // format!("obf_{}", hex_encoded)
}
