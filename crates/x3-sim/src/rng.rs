//! Deterministic randomness.
//!
//! Every draw comes from one seeded ChaCha8 stream that the simulator owns, so
//! a run is reproducible from `seed` alone. Nothing in the simulator may call
//! `OsRng`, `SystemTime` or `thread_rng`; those are exactly the sources that
//! would make a failure impossible to replay.

use rand::{Rng, SeedableRng};
use rand_chacha::ChaCha8Rng;

/// A seeded, counted random stream.
#[derive(Debug)]
pub struct SimRng {
    inner: ChaCha8Rng,
    draws: u64,
}

impl SimRng {
    pub fn from_seed(seed: u64) -> Self {
        Self {
            inner: ChaCha8Rng::seed_from_u64(seed),
            draws: 0,
        }
    }

    /// Number of values drawn so far. Also a cheap fingerprint of the schedule.
    pub fn draws(&self) -> u64 {
        self.draws
    }

    pub fn next_u64(&mut self) -> u64 {
        self.draws += 1;
        self.inner.gen()
    }

    /// Uniform-ish in `[0, n)`. `n == 0` yields `0` rather than dividing by zero.
    pub fn below(&mut self, n: usize) -> usize {
        if n == 0 {
            0
        } else {
            (self.next_u64() % n as u64) as usize
        }
    }

    /// True with the given percentage probability (`0..=100`).
    pub fn chance(&mut self, percent: u32) -> bool {
        (self.next_u64() % 100) < u64::from(percent.min(100))
    }

    /// 32 deterministic bytes.
    pub fn bytes32(&mut self) -> [u8; 32] {
        let mut out = [0u8; 32];
        for chunk in out.chunks_mut(8) {
            let word = self.next_u64().to_le_bytes();
            chunk.copy_from_slice(&word[..chunk.len()]);
        }
        out
    }
}
