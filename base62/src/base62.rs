use crate::{STATIC_BYTE_MAP, hash};

/// Base-62 encoder/decoder with a deterministic per-key character permutation.
pub struct Base62 {
    charset: [char; 62],
}

pub const BASE62_CHARS: [char; 62] = [
    '0', '1', '2', '3', '4', '5', '6', '7', '8', '9', 'A', 'B', 'C', 'D', 'E', 'F', 'G', 'H', 'I',
    'J', 'K', 'L', 'M', 'N', 'O', 'P', 'Q', 'R', 'S', 'T', 'U', 'V', 'W', 'X', 'Y', 'Z', 'a', 'b',
    'c', 'd', 'e', 'f', 'g', 'h', 'i', 'j', 'k', 'l', 'm', 'n', 'o', 'p', 'q', 'r', 's', 't', 'u',
    'v', 'w', 'x', 'y', 'z',
];

/// `8.0 / log2(62.0)`, used to estimate encoded length from a byte length.
const ENCODING_RATIO: f64 = 8.0 / 5.954196310386875; // 8.0 / log2(62.0)

impl Base62 {
    /// Builds the charset permutation for `key` and `nonce`.
    pub fn new(key: &[u8], nonce: usize) -> Self {
        // Re-hash the caller-provided key so charset generation always runs on a fixed-width input.
        let key = hash(key);

        let mut charset: [char; 62] = [0 as char; 62];

        let mut available_positions = (0..62).collect::<Vec<usize>>();

        for (char_index, ch) in BASE62_CHARS.iter().copied().enumerate() {
            let random_byte = STATIC_BYTE_MAP[(key[char_index % key.len()] as usize + nonce) % 255];

            let choice_index = random_byte as usize % available_positions.len();
            let charset_index = available_positions.remove(choice_index);

            charset[charset_index] = ch;
        }

        Self { charset }
    }

    /// Converts a character to its base-62 value in this instance's charset.
    fn char_to_value(&self, ch: char) -> Result<u8, String> {
        self.charset
            .iter()
            .position(|&c| c == ch)
            .map(|pos| pos as u8)
            .ok_or_else(|| format!("Invalid character for this charset: '{}'", ch))
    }

    /// Encodes a byte slice into a base-62 string using a custom character set
    /// while preserving leading zero bytes.
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

        // Repeated division keeps the implementation independent from bigint crates.
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
    /// while preserving leading zero bytes.
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

        // Rebuild the big-endian integer via repeated multiply-add.
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

    /// Encodes `data` using the nonce convention shared with [`decode_full`].
    pub fn encode_full(data: &[u8], key: &[u8]) -> String {
        let predicted_len = predict_base62_len(data);

        let base = Base62::new(key, predicted_len % 255);
        let encoded = base.encode(data);

        // The charset nonce is derived from the final encoded length, so a misprediction must
        // trigger one more pass with the actual length-derived nonce.
        if encoded.len() != predicted_len {
            let actual_len = encoded.len();
            let base = Base62::new(key, actual_len % 255);
            let encoded = base.encode(data);

            assert_eq!(encoded.len(), actual_len);

            encoded
        } else {
            encoded
        }
    }

    /// Decodes a string previously produced by [`encode_full`].
    pub fn decode_full(data: &str, key: &[u8]) -> Result<Vec<u8>, String> {
        let base = Base62::new(key, data.len() % 255);
        base.decode(data)
    }
}

/// Returns whether the big-endian integer represented by `num` is zero.
fn is_zero(num: &[u8]) -> bool {
    num.iter().all(|&b| b == 0)
}

/// Divides an in-place big-endian integer by `62`, returning the remainder.
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

    // Keep a canonical representation so the next loop iteration can stop at `[0]`.
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

/// Multiplies an in-place big-endian integer by `multiplier` and adds `add`.
fn mul_add(num: &mut Vec<u8>, multiplier: u16, add: u8) {
    let mut carry = add as u16;

    for byte in num.iter_mut().rev() {
        let product = (*byte as u16) * multiplier + carry;
        *byte = (product & 0xFF) as u8;
        carry = product >> 8;
    }

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

    let num_leading_zeros = input_bytes.iter().take_while(|&&b| b == 0).count();
    let num_rest_bytes = input_bytes.len() - num_leading_zeros;

    if num_rest_bytes == 0 {
        num_leading_zeros
    } else {
        let rest_len = (num_rest_bytes as f64 * ENCODING_RATIO).ceil();
        num_leading_zeros + rest_len as usize
    }
}
