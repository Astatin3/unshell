use crate::{base62::Base62, hash};
use aes::cipher::{BlockDecryptMut, BlockEncryptMut, KeyIvInit};
use cbc::cipher::block_padding::Pkcs7;
use regex::Regex;

fn pkcs7_padded_length(input_len: usize) -> usize {
    let block_size = 16;
    ((input_len / block_size) + 1) * block_size
}

pub fn encrypt_aes(plaintext: &str, key_str: &str, iv: [u8; 16]) -> String {
    let plaintext = plaintext.as_bytes();

    // Hash the env key to get a 32-byte (256-bit) AES key
    let key = hash(key_str.as_bytes());

    // Generate a psudo-random salt byte based on the plaintext
    // I hope this does not break the encryption.
    let mut salt = 0;

    for byte in plaintext {
        salt ^= byte;
    }

    let mut key_salted = key.clone();

    // Salt the key by XORing the salt byte with all the key bytes.
    // This ensures that the "hash" generated from the plaintext will
    // make the encrypted result extremely different.
    for i in 0..32 {
        key_salted[i] ^= salt;
    }

    let buf_len = pkcs7_padded_length(plaintext.len());

    let mut buf = vec![0u8; buf_len];
    let pt_len = plaintext.len();
    buf[..pt_len].copy_from_slice(&plaintext);

    let mut ct = cbc::Encryptor::<aes::Aes256>::new(&key_salted.into(), &iv.into())
        .encrypt_padded_mut::<Pkcs7>(&mut buf, pt_len)
        .unwrap()
        .to_vec();

    // Add the salt byte to the key byte,
    ct.insert(0, salt);

    // Encode result in base62
    Base62::encode_full(&ct, &key)
}

pub fn encrypt_aes_lines(plaintext: &str, key_str: &str, iv: [u8; 16]) -> String {
    format!("_{}_", encrypt_aes(plaintext, key_str, iv))
}

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

    for aes_block in Regex::new(r"_([0-9a-zA-Z]*?)_").unwrap().find_iter(&input) {
        let range = aes_block.range();
        let aes_block = aes_block.as_str()[1..(aes_block.len() - 1)].to_string();

        if let Ok(decrypted_block) = decrypt_aes(&aes_block, key_str, iv) {
            let range = (range.start + total_offset as usize)..(range.end + total_offset as usize);

            // Offset range by the difference between the decrypted block length and the original range length
            total_offset += decrypted_block.len().clone() - (range.end - range.start);

            decrypted_result.replace_range(range, &decrypted_block);
        } else {
            continue;
        }
    }

    decrypted_result
}
