use crate::{base62::Base62, hash};
use aes::cipher::{BlockEncryptMut, KeyIvInit};
use cbc::cipher::block_padding::Pkcs7;

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
