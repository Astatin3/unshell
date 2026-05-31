use crate::crypto::feistel_shuffle;

#[cfg(feature = "counter_shuffle_none")]
pub type Counter = NoShuffle;
#[cfg(feature = "counter_shuffle_feistel")]
pub type Counter = FeistelShuffle;
#[cfg(feature = "counter_shuffle_feistel_lcg")]
pub type Counter = FeistelLCGShuffle;

const NONCE16_1: u16 = const_random::const_random!(u16);
const NONCE16_2: u16 = const_random::const_random!(u16);
const NONCE32: u32 = const_random::const_random!(u32);

pub struct NoShuffle(u16);

/// Linear shuffle, no randomization, just a random starting point and step size
impl NoShuffle {
    pub fn new() -> Self {
        Self(NONCE16_1)
    }

    pub fn next(&mut self) -> u16 {
        self.0 = self.0.wrapping_add(1);
        self.0
    }
}

/// Shuffle all 16 bit numbers, an actual shuffle
/// But this still stores local values in a linear format
pub struct FeistelShuffle(u16, u32);

impl FeistelShuffle {
    pub fn new() -> Self {
        Self(NONCE16_1, NONCE32)
    }

    pub fn next(&mut self) -> u16 {
        self.0 = self.0.wrapping_add(NONCE16_2);
        feistel_shuffle(self.0, self.1)
    }
}

/// Linear recursive shuffle,
/// feeds back into itself and doesn't store the actual state.
/// Harder to decompile
pub struct FeistelLCGShuffle {
    state: u16,
    a: u16, // Multiplier (must be 1 mod 4)
    c: u16, // Increment (must be odd)
}

impl FeistelLCGShuffle {
    pub fn new() -> Self {
        let seed = NONCE32;
        let a = (((seed & 0x3FFF) as u16) << 2) | 1;
        let c = ((seed >> 16) as u16) | 1;
        Self { state: 0, a, c }
    }

    pub fn next(&mut self) -> u16 {
        // 1. Advance state using LCG (Guarantees single cycle of 65536)
        self.state = self.state.wrapping_mul(self.a).wrapping_add(self.c);

        // 2. Apply Feistel shuffle to the state (Adds randomness)
        feistel_shuffle(self.state, self.a as u32)
    }
}
