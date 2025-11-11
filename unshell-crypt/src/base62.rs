use crate::{STATIC_BYTE_MAP, hash};

// Randomly mapped Base62 characters
pub struct Base62 {
    charset: [char; 62],
}

pub const BASE62_CHARS: [char; 62] = [
    '0', '1', '2', '3', '4', '5', '6', '7', '8', '9', 'A', 'B', 'C', 'D', 'E', 'F', 'G', 'H', 'I',
    'J', 'K', 'L', 'M', 'N', 'O', 'P', 'Q', 'R', 'S', 'T', 'U', 'V', 'W', 'X', 'Y', 'Z', 'a', 'b',
    'c', 'd', 'e', 'f', 'g', 'h', 'i', 'j', 'k', 'l', 'm', 'n', 'o', 'p', 'q', 'r', 's', 't', 'u',
    'v', 'w', 'x', 'y', 'z',
];

// Const for ratio
const ENCODING_RATIO: f64 = 8.0 / 5.954196310386875; // 8.0 / log2(62.0)

impl Base62 {
    pub fn new(key: &[u8], nonce: usize) -> Self {
        // Hash key again, for the chance that this random function can be used to derive the key
        let key = hash(key);

        let mut charset: [char; 62] = [0 as char; 62];

        // Create a vector of indices from 0 to 61
        let mut current_indicies = (0..62).map(|i| i as usize).collect::<Vec<usize>>();

        // Loop through each byte in the key until all chars are filled
        for i in 0..62 as usize {
            let rand = STATIC_BYTE_MAP[(key[i as usize % key.len()] as usize + nonce) % 255];

            let index_index = rand as usize % current_indicies.len();
            let put_index = current_indicies.remove(index_index);

            charset[put_index] = BASE62_CHARS[i];
        }

        return Self { charset };
    }

    // Convert character to base-62 value using custom charset
    fn char_to_value(&self, ch: char) -> Result<u8, String> {
        self.charset
            .iter()
            .position(|&c| c == ch)
            .map(|pos| pos as u8)
            .ok_or_else(|| format!("Invalid character for this charset: '{}'", ch))
    }

    /// Encodes a byte slice into a base-62 string using a custom character set
    /// Supports arbitrary length input by using big integer arithmetic
    pub fn encode(&self, data: &[u8]) -> String {
        if data.is_empty() {
            return String::new();
        }

        // Count leading zeros
        let leading_zeros = data.iter().take_while(|&&b| b == 0).count();

        // Skip leading zeros for conversion
        let data = &data[leading_zeros..];

        if data.is_empty() {
            return self.charset[0].to_string().repeat(leading_zeros);
        }

        let mut result = Vec::new();
        let mut num = data.to_vec();

        // Convert to base-62 using division
        while !is_zero(&num) {
            let remainder = div_mod_62(&mut num);
            result.push(self.charset[remainder]);
        }

        // Add leading zeros
        for _ in 0..leading_zeros {
            result.push(self.charset[0]);
        }

        // Reverse since we built it backwards
        result.reverse();
        result.into_iter().collect()
    }

    /// Decodes a base-62 string back into bytes using a custom character set
    /// Supports arbitrary length output
    pub fn decode(&self, encoded: &str) -> Result<Vec<u8>, String> {
        if encoded.is_empty() {
            return Ok(Vec::new());
        }

        // Count leading zeros (first character in charset)
        let zero_char = self.charset[0];
        let leading_zeros = encoded.chars().take_while(|&c| c == zero_char).count();

        // Skip leading zeros for conversion
        let encoded = &encoded[leading_zeros..];

        if encoded.is_empty() {
            return Ok(vec![0; leading_zeros]);
        }

        // Convert base-62 string to bytes using multiplication
        let mut num = vec![0u8];

        for ch in encoded.chars() {
            let value = self.char_to_value(ch)?;
            mul_add(&mut num, 62, value);
        }

        // Add leading zero bytes
        let mut result = vec![0u8; leading_zeros];
        result.append(&mut num);

        Ok(result)
    }

    pub fn encode_full(data: &[u8], key: &[u8]) -> String {
        // Predict the length of the encoded data
        let length = predict_base62_len(data);

        let base = Base62::new(&key, length % 255);
        let encoded = base.encode(data);

        if encoded.len() != length {
            let len = encoded.len();
            let base = Base62::new(&key, len % 255);
            let encoded = base.encode(data);

            println!("Fallback");

            assert_eq!(encoded.len(), len);

            encoded
        } else {
            encoded
        }
    }
    pub fn decode_full(data: &str, key: &[u8]) -> Result<Vec<u8>, String> {
        let base = Base62::new(&key, data.len() % 255);
        base.decode(data)
    }

    // pub fn encode_full
}

// Helper: Check if big integer (as bytes) is zero
fn is_zero(num: &[u8]) -> bool {
    num.iter().all(|&b| b == 0)
}

// Helper: Divide big integer by 62 and return remainder
// Modifies num in place to be the quotient
fn div_mod_62(num: &mut Vec<u8>) -> usize {
    let mut remainder = 0u16;
    let mut all_zero = true;

    for byte in num.iter_mut() {
        let current = (remainder << 8) | (*byte as u16);
        *byte = (current / 62) as u8;
        remainder = current % 62;
        if *byte != 0 {
            all_zero = false;
        }
    }

    // Remove leading zeros from quotient
    if all_zero {
        num.clear();
        num.push(0);
    } else {
        let first_nonzero = num.iter().position(|&b| b != 0).unwrap_or(0);
        if first_nonzero > 0 {
            num.drain(0..first_nonzero);
        }
    }

    remainder as usize
}

// Helper: Multiply big integer by 62 and add a value
// Modifies num in place
fn mul_add(num: &mut Vec<u8>, multiplier: u16, add: u8) {
    let mut carry = add as u16;

    for byte in num.iter_mut().rev() {
        let product = (*byte as u16) * multiplier + carry;
        *byte = (product & 0xFF) as u8;
        carry = product >> 8;
    }

    // Add remaining carry bytes
    while carry > 0 {
        num.insert(0, (carry & 0xFF) as u8);
        carry >>= 8;
    }
}

/// Predicts the byte length of the decoded output given a base-62 encoded string
/// This calculates the length without performing the full decoding
pub fn predict_base62_len(input_bytes: &[u8]) -> usize {
    if input_bytes.is_empty() {
        return 0;
    }

    // 1. Count leading zero bytes.
    let num_leading_zeros = input_bytes.iter().take_while(|&&b| b == 0).count();

    // 2. Calculate length of the rest of the bytes.
    let num_rest_bytes = input_bytes.len() - num_leading_zeros;

    if num_rest_bytes == 0 {
        // If all bytes were zeros, the length is just the number of zeros.
        num_leading_zeros
    } else {
        // 3. Calculate the mathematical upper bound for the non-zero part.
        // This is ceil(num_rest_bytes * 8_bits / log2(62))
        // which is ceil(num_rest_bytes * log_62(256))
        let rest_len = (num_rest_bytes as f64 * ENCODING_RATIO).ceil();

        // 4. Total length is zeros + rest_len
        num_leading_zeros + rest_len as usize
    }
}
