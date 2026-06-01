use crate::crypto::feistel_shuffle;

/// Counter implementation selected by feature flags.
///
/// Cargo's `--all-features` enables every counter strategy at once, so these cfgs are
/// intentionally priority-ordered instead of mutually exclusive aliases. The strongest
/// configured shuffle wins: Feistel+LCG, then Feistel, then the linear fallback.
#[cfg(all(
    feature = "counter_shuffle_none",
    not(any(
        feature = "counter_shuffle_feistel",
        feature = "counter_shuffle_feistel_lcg"
    ))
))]
pub type Counter = NoShuffle;

/// Counter implementation selected when Feistel is enabled without Feistel+LCG.
#[cfg(all(
    feature = "counter_shuffle_feistel",
    not(feature = "counter_shuffle_feistel_lcg")
))]
pub type Counter = FeistelShuffle;

/// Default and strongest counter implementation.
#[cfg(feature = "counter_shuffle_feistel_lcg")]
pub type Counter = FeistelLCGShuffle;

/// Fallback used only when all counter shuffle features are disabled.
#[cfg(not(any(
    feature = "counter_shuffle_none",
    feature = "counter_shuffle_feistel",
    feature = "counter_shuffle_feistel_lcg"
)))]
pub type Counter = NoShuffle;

const NONCE16_1: u16 = const_random::const_random!(u16);
const NONCE16_2: u16 = const_random::const_random!(u16);
const NONCE32: u32 = const_random::const_random!(u32);

/// Odd additive step used by [`FeistelShuffle`] before applying the permutation.
///
/// A step through a `u16` counter only visits every possible value when it is
/// coprime with `2^16`; for powers of two, that means the step must be odd. Without
/// this constraint, a randomized even step can cycle through a subset of values and
/// collide before the hook id space is exhausted.
const FEISTEL_STEP: u16 = NONCE16_2 | 1;

pub struct NoShuffle(u16);

/// Linear shuffle, no randomization, just a random starting point and step size
impl NoShuffle {
    pub fn new() -> Self {
        Self(NONCE16_1)
    }

    // This is an id generator API, not an iterator: callers need a bare `u16` and no
    // exhaustion state because the counter intentionally wraps through the full space.
    #[allow(clippy::should_implement_trait)]
    pub fn next(&mut self) -> u16 {
        self.0 = self.0.wrapping_add(1);
        self.0
    }
}

impl Default for NoShuffle {
    fn default() -> Self {
        Self::new()
    }
}

/// Shuffle all 16 bit numbers, an actual shuffle
/// But this still stores local values in a linear format
pub struct FeistelShuffle(u16, u32);

impl FeistelShuffle {
    pub fn new() -> Self {
        Self(NONCE16_1, NONCE32)
    }

    // This is an id generator API, not an iterator: callers need a bare `u16` and no
    // exhaustion state because the counter intentionally wraps through the full space.
    #[allow(clippy::should_implement_trait)]
    pub fn next(&mut self) -> u16 {
        self.0 = self.0.wrapping_add(FEISTEL_STEP);
        feistel_shuffle(self.0, self.1)
    }
}

impl Default for FeistelShuffle {
    fn default() -> Self {
        Self::new()
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

    // This is an id generator API, not an iterator: callers need a bare `u16` and no
    // exhaustion state because the counter intentionally wraps through the full space.
    #[allow(clippy::should_implement_trait)]
    pub fn next(&mut self) -> u16 {
        // 1. Advance state using LCG (Guarantees single cycle of 65536)
        self.state = self.state.wrapping_mul(self.a).wrapping_add(self.c);

        // 2. Apply Feistel shuffle to the state (Adds randomness)
        feistel_shuffle(self.state, self.a as u32)
    }
}

impl Default for FeistelLCGShuffle {
    fn default() -> Self {
        Self::new()
    }
}
