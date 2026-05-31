use alloc::string::String;

// TODO: Make this seed dependent on env var;
pub const GLOBAL_SEED: u32 = 0xDEAFBEEF;
// pub const GLOBAL_NONCE: u32 = {
//     let time = match u128::from_str_radix(env!("BUILD_TIME"), 10) {
//         Ok(i) => i,
//         Err(_) => panic!("Failed to parse BUILD_TIME"),
//     };

//     GLOBAL_SEED ^ (time as u32)
// };

mod feistel;
#[allow(dead_code)]
mod feistel_state;
mod sha256;

pub use feistel::feistel_shuffle;
pub use feistel_state::{Counter, FeistelLCGShuffle, FeistelShuffle, NoShuffle};
pub use sha256::sha256;

#[cfg(test)]
mod tests;

#[macro_export]
macro_rules! hash_256 {
    ($s:literal) => {{
        // string literal arm
        const HASH: [u8; 32] = $crate::crypto::sha256($s.as_bytes());
        HASH
    }};
    ($n:expr) => {{
        // integer/expression arm
        const BYTES: [u8; 8] = ($n as u64).to_be_bytes();
        const HASH: [u8; 32] = $crate::crypto::sha256(&BYTES);
        HASH
    }};
}

#[macro_export]
macro_rules! hash_32 {
    ($s:literal) => {{
        // string literal arm
        const HASH: [u8; 32] = $crate::crypto::sha256($s.as_bytes());
        const RESULT: u32 = u32::from_be_bytes([HASH[0], HASH[8], HASH[16], HASH[24]]);
        RESULT
    }};
    ($n:expr) => {{
        // integer/expression arm
        const BYTES: [u8; 8] = ($n as u64).to_be_bytes();
        const HASH: [u8; 32] = $crate::crypto::sha256(&BYTES);
        const RESULT: u32 = u32::from_be_bytes([HASH[0], HASH[8], HASH[16], HASH[24]]);
        RESULT
    }};
}

pub fn hash_string_32(input: String) -> u32 {
    let hash: [u8; 32] = sha256(input.as_bytes());
    u32::from_be_bytes([hash[0], hash[8], hash[16], hash[24]])
}

pub fn hash_str_32(input: &str) -> u32 {
    let hash: [u8; 32] = sha256(input.as_bytes());
    u32::from_be_bytes([hash[0], hash[8], hash[16], hash[24]])
}

pub fn hash_32(input: u32) -> u32 {
    let hash: [u8; 32] = sha256(&input.to_be_bytes());
    u32::from_be_bytes([hash[0], hash[8], hash[16], hash[24]])
}
