//! Virtual clock.
//!
//! The simulator never reads the wall clock. Time only moves when the
//! simulation advances it, which is what lets a run with `--seed N` and a run
//! that replays `N` produce byte-identical traces.

/// Millisecond-resolution virtual clock.
///
/// Milliseconds are kept internally so that sub-second network latency is
/// expressible; the coordinator is only ever handed whole seconds.
#[derive(Debug, Clone, Copy)]
pub struct VirtualClock {
    now_ms: u64,
}

impl VirtualClock {
    pub fn new(start_ms: u64) -> Self {
        Self { now_ms: start_ms }
    }

    pub fn now_ms(&self) -> u64 {
        self.now_ms
    }

    pub fn now_secs(&self) -> u64 {
        self.now_ms / 1_000
    }

    /// Move time forward and return the new instant.
    ///
    /// Time never moves backwards; the spawn/add semantics of a real node
    /// (NTP step, clock skew) are modelled by latency faults, not by rewinding
    /// this clock, because rewinding would make the RNG draws diverge from the
    /// recorded trace.
    pub fn advance(&mut self, delta_ms: u64) -> u64 {
        self.now_ms = self.now_ms.saturating_add(delta_ms);
        self.now_ms
    }
}
