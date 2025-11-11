use crate::{base62::Base62, hash};
use aes::cipher::{BlockDecryptMut, BlockEncryptMut, KeyIvInit};
use cbc::cipher::block_padding::Pkcs7;
use regex::Regex;

fn pkcs7_padded_length(input_len: usize) -> usize {
    let block_size = 16;
    ((input_len / block_size) + 1) * block_size
}

pub fn encrypt_aes(plaintext: &str, key_str: &str, iv: [u8; 16]) -> String {
    // Hash the env key to get a 32-byte (256-bit) AES key
    let key = hash(key_str.as_bytes());

    let plaintext = plaintext.as_bytes();

    let buf_len = pkcs7_padded_length(plaintext.len());

    let mut buf = vec![0u8; buf_len];
    let pt_len = plaintext.len();
    buf[..pt_len].copy_from_slice(&plaintext);
    let ct = cbc::Encryptor::<aes::Aes256>::new(&key.into(), &iv.into())
        .encrypt_padded_mut::<Pkcs7>(&mut buf, pt_len)
        .unwrap();

    Base62::encode_full(ct, &key)
}

pub fn encrypt_aes_lines(plaintext: &str, key_str: &str, iv: [u8; 16]) -> String {
    format!("_{}_", encrypt_aes(plaintext, key_str, iv))
}

pub fn decrypt_aes(input: &str, key_str: &str, iv: [u8; 16]) -> String {
    // Hash the env key to get a 32-byte (256-bit) AES key
    let key = hash(key_str.as_bytes());

    let cipher_bytes = Base62::decode_full(input, &key).unwrap();

    // Create buffer for result
    let buf_len = cipher_bytes.len();
    let mut buf: Vec<u8> = vec![0; buf_len];
    buf[..cipher_bytes.len()].copy_from_slice(&cipher_bytes);

    if let Ok(pt) = cbc::Decryptor::<aes::Aes256>::new(&key.into(), &iv.into())
        .decrypt_padded_mut::<Pkcs7>(&mut buf)
    {
        String::from_utf8_lossy(pt).to_string()
    } else {
        "<decryption failed>".to_string()
    }
}

pub fn decrypt_aes_lines(input: &str, key_str: &str, iv: [u8; 16]) -> String {
    let mut decrypted_result = input.to_string();
    let mut total_offset = 0;

    for aes_block in Regex::new(r"_([0-9a-zA-Z]*?)_").unwrap().find_iter(&input) {
        let range = aes_block.range();
        let aes_block = aes_block.as_str()[1..(aes_block.len() - 1)].to_string();
        let decrypted_block = decrypt_aes(&aes_block, key_str, iv);

        let range = (range.start + total_offset as usize)..(range.end + total_offset as usize);

        // Offset range by the difference between the decrypted block length and the original range length
        total_offset += decrypted_block.len().clone() - (range.end - range.start);

        decrypted_result.replace_range(range, &decrypted_block);
    }

    decrypted_result
}
