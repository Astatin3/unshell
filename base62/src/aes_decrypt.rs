use aes::cipher::{BlockDecryptMut, KeyIvInit, block_padding::Pkcs7};
use regex::Regex;

use crate::{Base62, hash};

pub fn decrypt_aes(input: &str, key_str: &str, iv: [u8; 16]) -> Result<String, String> {
    // Hash the env key to get a 32-byte (256-bit) AES key
    let mut key = hash(key_str.as_bytes());

    let mut cipher_bytes = Base62::decode_full(input, &key).unwrap();

    let salt = cipher_bytes.remove(0);

    // XOR the salt bytes with the key bytes
    // This replicates
    for i in 0..32 {
        key[i] ^= salt;
    }

    // Create buffer for result
    let buf_len = cipher_bytes.len();
    let mut buf: Vec<u8> = vec![0; buf_len];
    buf[..cipher_bytes.len()].copy_from_slice(&cipher_bytes);

    let pt = cbc::Decryptor::<aes::Aes256>::new(&key.into(), &iv.into())
        .decrypt_padded_mut::<Pkcs7>(&mut buf)
        .map_err(|_| "decryption failed".to_string())?;

    Ok(String::from_utf8_lossy(pt).to_string())
}

pub fn decrypt_aes_lines(input: &str, key_str: &str, iv: [u8; 16]) -> String {
    let mut decrypted_result = input.to_string();
    let mut total_offset = 0;

    // Split input by segments of base62 chars, denoted by two _'s, and attempt to decode
    for aes_block in Regex::new(r"_([0-9a-zA-Z]*?)_").unwrap().find_iter(&input) {
        let range = aes_block.range();
        let aes_block = aes_block.as_str()[1..(aes_block.len() - 1)].to_string();

        // If the decryption is successful, offset the current offset position
        if let Ok(decrypted_block) = decrypt_aes(&aes_block, key_str, iv) {
            let range = (range.start + total_offset as usize)..(range.end + total_offset as usize);

            // Offset range by the difference between the decrypted block length and the original range length
            total_offset += decrypted_block.len().clone() - (range.end - range.start);

            decrypted_result.replace_range(range, &decrypted_block);
        } else {
            // If the decode is unsuccessful, leave the underscore-denoted region as is
            continue;
        }
    }

    decrypted_result
}
