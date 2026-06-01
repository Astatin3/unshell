/// Performs a deterministic pseudo-random shuffle of a 16-bit index.
///
/// # Arguments
/// * `index` - The input value (0..65536).
/// * `seed` - The 32-bit seed acting as the encryption key.
///
/// # Returns
/// A unique 16-bit shuffled value.
pub fn feistel_shuffle(index: u16, seed: u32) -> u16 {
    // Split 16-bit index into two 8-bit halves
    let mut l = ((index >> 8) & 0xFF) as u8;
    let mut r = (index & 0xFF) as u8;

    // Perform 4 rounds of Feistel mixing
    for round in 0..4 {
        // Derive sub-key: Rotate seed and add golden ratio constant
        let rot_amount = (round * 5) % 32;
        let sub_key = seed
            .rotate_left(rot_amount)
            .wrapping_add(round.wrapping_mul(0x9E3779B9));

        // Round function F: Simple multiplicative hash mixing R and sub_key
        // We cast to u32 for multiplication to avoid overflow, then mask back to 8 bits
        let r_u32 = r as u32;
        let hash_val = ((r_u32.wrapping_mul(sub_key)) ^ (r_u32 >> 4)) as u8 & 0xFF;

        // Feistel step: New L = Old R, New R = Old L XOR F(R, key)
        let temp = l;
        l = r;
        r = temp ^ hash_val;
    }

    // Recombine halves
    ((l as u16) << 8) | (r as u16)
}
