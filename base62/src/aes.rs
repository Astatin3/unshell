use crate::{base62::Base62, hash};
use aes::cipher::{BlockModeDecrypt, BlockModeEncrypt, KeyIvInit};
use cbc::cipher::block_padding::Pkcs7;
use regex::Regex;

/// Returns the next AES block-sized buffer length for PKCS#7 padding.
fn pkcs7_padded_length(input_len: usize) -> usize {
    let block_size = 16;
    ((input_len / block_size) + 1) * block_size
}

fn xor_fold(bytes: &[u8]) -> u8 {
    bytes.iter().fold(0, |salt, &byte| salt ^ byte)
}

fn xor_key_with_salt(key: &mut [u8; 32], salt: u8) {
    for byte in key.iter_mut() {
        *byte ^= salt;
    }
}

/// Encrypts `plaintext` with AES-256-CBC and base62-encodes the result.
pub fn encrypt_aes(plaintext: &str, key_str: &str, iv: [u8; 16]) -> String {
    let plaintext = plaintext.as_bytes();

    // Hash the environment key into the fixed-width AES-256 key material expected below.
    let key = hash(key_str.as_bytes());

    // This crate intentionally perturbs the key with a single plaintext-derived byte so
    // similar inputs diverge quickly after encryption while remaining reversible.
    let salt = xor_fold(plaintext);

    let mut salted_key = key;
    xor_key_with_salt(&mut salted_key, salt);

    let buf_len = pkcs7_padded_length(plaintext.len());

    let mut buf = vec![0u8; buf_len];
    let pt_len = plaintext.len();
    buf[..pt_len].copy_from_slice(plaintext);

    let mut ciphertext = cbc::Encryptor::<aes::Aes256>::new(&salted_key.into(), &iv.into())
        .encrypt_padded::<Pkcs7>(&mut buf, pt_len)
        .unwrap()
        .to_vec();

    // Prefix the salt so decryption can rebuild the same salted key.
    ciphertext.insert(0, salt);

    Base62::encode_full(&ciphertext, &key)
}

/// Wraps [`encrypt_aes`] output in `_..._` markers for line-wise replacement.
pub fn encrypt_aes_lines(plaintext: &str, key_str: &str, iv: [u8; 16]) -> String {
    format!("_{}_", encrypt_aes(plaintext, key_str, iv))
}

/// Decodes base62 input and decrypts the payload with AES-256-CBC.
pub fn decrypt_aes(input: &str, key_str: &str, iv: [u8; 16]) -> Result<String, String> {
    let mut key = hash(key_str.as_bytes());

    let mut cipher_bytes = Base62::decode_full(input, &key)?;

    if cipher_bytes.is_empty() {
        return Err("decryption failed".to_string());
    }

    let salt = cipher_bytes.remove(0);

    xor_key_with_salt(&mut key, salt);

    let buf_len = cipher_bytes.len();
    let mut buf: Vec<u8> = vec![0; buf_len];
    buf[..cipher_bytes.len()].copy_from_slice(&cipher_bytes);

    let pt = cbc::Decryptor::<aes::Aes256>::new(&key.into(), &iv.into())
        .decrypt_padded::<Pkcs7>(&mut buf)
        .map_err(|_| "decryption failed".to_string())?;

    Ok(String::from_utf8_lossy(pt).to_string())
}

/// Replaces every `_..._` AES marker in `input` that decrypts successfully.
pub fn decrypt_aes_lines(input: &str, key_str: &str, iv: [u8; 16]) -> String {
    let mut decrypted_result = input.to_string();
    let mut replacement_offset: isize = 0;

    // Walk the original input and patch into a separate mutable buffer. The offset keeps the
    // current match aligned after prior replacements change the string length.
    for aes_block in Regex::new(r"_([0-9a-zA-Z]*?)_").unwrap().find_iter(input) {
        let range = aes_block.range();
        let aes_block = aes_block.as_str()[1..(aes_block.len() - 1)].to_string();

        if let Ok(decrypted_block) = decrypt_aes(&aes_block, key_str, iv) {
            let adjusted_start = (range.start as isize + replacement_offset) as usize;
            let adjusted_end = (range.end as isize + replacement_offset) as usize;
            let adjusted_range = adjusted_start..adjusted_end;

            replacement_offset += decrypted_block.len() as isize - adjusted_range.len() as isize;

            decrypted_result.replace_range(adjusted_range, &decrypted_block);
        }
    }

    decrypted_result
}
