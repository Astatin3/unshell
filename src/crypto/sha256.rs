// ── Round constants ──────────────────────────────────────────────────────────
const K: [u32; 64] = [
    0x428A2F98, 0x71374491, 0xB5C0FBCF, 0xE9B5DBA5, 0x3956C25B, 0x59F111F1, 0x923F82A4, 0xAB1C5ED5,
    0xD807AA98, 0x12835B01, 0x243185BE, 0x550C7DC3, 0x72BE5D74, 0x80DEB1FE, 0x9BDC06A7, 0xC19BF174,
    0xE49B69C1, 0xEFBE4786, 0x0FC19DC6, 0x240CA1CC, 0x2DE92C6F, 0x4A7484AA, 0x5CB0A9DC, 0x76F988DA,
    0x983E5152, 0xA831C66D, 0xB00327C8, 0xBF597FC7, 0xC6E00BF3, 0xD5A79147, 0x06CA6351, 0x14292967,
    0x27B70A85, 0x2E1B2138, 0x4D2C6DFC, 0x53380D13, 0x650A7354, 0x766A0ABB, 0x81C2C92E, 0x92722C85,
    0xA2BFE8A1, 0xA81A664B, 0xC24B8B70, 0xC76C51A3, 0xD192E819, 0xD6990624, 0xF40E3585, 0x106AA070,
    0x19A4C116, 0x1E376C08, 0x2748774C, 0x34B0BCB5, 0x391C0CB3, 0x4ED8AA4A, 0x5B9CCA4F, 0x682E6FF3,
    0x748F82EE, 0x78A5636F, 0x84C87814, 0x8CC70208, 0x90BEFFFA, 0xA4506CEB, 0xBEF9A3F7, 0xC67178F2,
];

// ── Initial hash values ──────────────────────────────────────────────────────
const H: [u32; 8] = [
    0x6A09E667, 0xBB67AE85, 0x3C6EF372, 0xA54FF53A, 0x510E527F, 0x9B05688C, 0x1F83D9AB, 0x5BE0CD19,
];
// ── Internals ────────────────────────────────────────────────────────────────

/// Returns what byte `pos` should hold in the padded SHA-256 message,
/// without ever materialising the full padded buffer.
const fn padded_byte(input: &[u8], pos: usize, padded_len: usize) -> u8 {
    let bit_len = (input.len() as u64) * 8;
    if pos < input.len() {
        input[pos]
    } else if pos == input.len() {
        0x80
    } else if pos >= padded_len - 8 {
        // Big-endian 64-bit length: byte 0 is the most significant.
        let byte_index = pos - (padded_len - 8);
        (bit_len >> (56 - byte_index * 8)) as u8
    } else {
        0x00
    }
}

/// SHA-256 compression: mixes one 64-byte block into the hash state.
const fn compress(state: &mut [u32; 8], block: &[u8; 64]) {
    // Build the 64-word message schedule from the 16-word block.
    let mut w = [0u32; 64];
    let mut i = 0;
    while i < 16 {
        w[i] = ((block[i * 4] as u32) << 24)
            | ((block[i * 4 + 1] as u32) << 16)
            | ((block[i * 4 + 2] as u32) << 8)
            | (block[i * 4 + 3] as u32);
        i += 1;
    }
    while i < 64 {
        let s0 = w[i - 15].rotate_right(7) ^ w[i - 15].rotate_right(18) ^ (w[i - 15] >> 3);
        let s1 = w[i - 2].rotate_right(17) ^ w[i - 2].rotate_right(19) ^ (w[i - 2] >> 10);
        w[i] = w[i - 16]
            .wrapping_add(s0)
            .wrapping_add(w[i - 7])
            .wrapping_add(s1);
        i += 1;
    }

    // Initialise working variables from current hash state.
    let mut a = state[0];
    let mut b = state[1];
    let mut c = state[2];
    let mut d = state[3];
    let mut e = state[4];
    let mut f = state[5];
    let mut g = state[6];
    let mut h = state[7];

    // 64 rounds.
    i = 0;
    while i < 64 {
        let sigma1 = e.rotate_right(6) ^ e.rotate_right(11) ^ e.rotate_right(25);
        let ch = (e & f) ^ ((!e) & g);
        let temp1 = h
            .wrapping_add(sigma1)
            .wrapping_add(ch)
            .wrapping_add(K[i])
            .wrapping_add(w[i]);

        let sigma0 = a.rotate_right(2) ^ a.rotate_right(13) ^ a.rotate_right(22);
        let maj = (a & b) ^ (a & c) ^ (b & c);
        let temp2 = sigma0.wrapping_add(maj);

        h = g;
        g = f;
        f = e;
        e = d.wrapping_add(temp1);
        d = c;
        c = b;
        b = a;
        a = temp1.wrapping_add(temp2);
        i += 1;
    }

    // Add the compressed chunk back into the hash state.
    state[0] = state[0].wrapping_add(a);
    state[1] = state[1].wrapping_add(b);
    state[2] = state[2].wrapping_add(c);
    state[3] = state[3].wrapping_add(d);
    state[4] = state[4].wrapping_add(e);
    state[5] = state[5].wrapping_add(f);
    state[6] = state[6].wrapping_add(g);
    state[7] = state[7].wrapping_add(h);
}

// ── Public API ───────────────────────────────────────────────────────────────

/// Returns the SHA-256 digest of `input` as 32 raw bytes.
pub const fn sha256(input: &[u8]) -> [u8; 32] {
    // Padded length is the next multiple of 64 that fits input + 1 (0x80) + 8 (length).
    let padded_len = (input.len() + 9).div_ceil(64) * 64;
    let mut state = H;
    let mut block_start = 0;

    while block_start < padded_len {
        // Assemble the current 64-byte block using the virtual padded view.
        let mut block = [0u8; 64];
        let mut j = 0;
        while j < 64 {
            block[j] = padded_byte(input, block_start + j, padded_len);
            j += 1;
        }
        compress(&mut state, &block);
        block_start += 64;
    }

    // Serialise the 8×u32 state as big-endian bytes.
    let mut out = [0u8; 32];
    let mut i = 0;
    while i < 8 {
        out[i * 4] = (state[i] >> 24) as u8;
        out[i * 4 + 1] = (state[i] >> 16) as u8;
        out[i * 4 + 2] = (state[i] >> 8) as u8;
        out[i * 4 + 3] = state[i] as u8;
        i += 1;
    }
    out
}
