use crate::crypto::{FeistelLCGShuffle, FeistelShuffle, NoShuffle};

#[test]
fn test_linear_shuffle() {
    let mut seen = [false; 65536];
    let mut counter = NoShuffle::new();
    for _ in 0..65535 {
        let val = counter.next();

        assert!(!seen[val as usize], "Collision detected");

        seen[val as usize] = true;
    }
}

#[test]
fn test_feistel_shuffle() {
    let mut seen = [false; 65536];
    let mut counter = FeistelShuffle::new();
    for _ in 0..65535 {
        let val = counter.next();

        assert!(!seen[val as usize], "Collision detected");

        seen[val as usize] = true;
    }
}

#[test]
fn test_fristel_lcg_shuffle() {
    let mut seen = [false; 65536];
    let mut counter = FeistelLCGShuffle::new();
    for _ in 0..65535 {
        let val = counter.next();

        assert!(!seen[val as usize], "Collision detected");

        seen[val as usize] = true;
    }
}
