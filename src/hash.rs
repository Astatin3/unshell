//! Temporary hash function

const fn hash_recursive(state: &mut [u8; 4], input: &[u8]) {
    match input.len() {
        3 => {
            state[0] ^= input[0];
            state[1] ^= input[1];
            state[2] ^= input[2];
        }
        2 => {
            state[0] ^= input[0];
            state[1] ^= input[1];
        }
        1 => {
            state[0] ^= input[0];
        }
        0 => {}
        _ => {
            state[0] ^= input[0];
            state[1] ^= input[1];
            state[2] ^= input[2];
            state[3] ^= input[3];

            // Mess with the state quite a bit
            state[0] = u8::reverse_bits(state[0]) ^ state[2];
            state[2] = state[0].wrapping_add(state[2]).wrapping_add(state[3]) ^ state[0];
            state[3] = state[2].wrapping_add(state[3] << 2) ^ state[1];
            state[1] = state[3] ^ 0xa3;

            hash_recursive(state, &input[1..]);
        }
    }
}

pub const fn hash(input: &'static str) -> u32 {
    let mut data = [0xDE, 0xED, 0xBE, 0xEF];
    hash_recursive(&mut data, input.as_bytes());

    // throw the data back into itself because why not
    let input2 = [
        u8::reverse_bits(data[1]),
        data[2],
        data[2],
        data[1],
        u8::reverse_bits(data[0]),
        data[2],
        u8::reverse_bits(data[3]),
        u8::reverse_bits(data[2]),
        data[3],
        u8::reverse_bits(data[3]),
        u8::reverse_bits(data[2]),
        data[0],
    ];
    hash_recursive(&mut data, &input2);

    u32::from_be_bytes(data)
}
