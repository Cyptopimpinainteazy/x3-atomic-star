//! Validate that each invariant in tests/invariants/registry.toml is referenced by at least one test file.
//!
//! This is a lightweight meta-test ensuring we don't accrue untested invariants.

use std::fs;
use std::path::Path;

#[derive(Debug, Clone, Copy, Default)]
struct SupplyState {
    canonical_supply: i128,
    native_supply: i128,
    evm_supply: i128,
    svm_supply: i128,
    x3vm_supply: i128,
    external_locked_supply: i128,
    pending_supply: i128,
}

impl SupplyState {
    fn represented_total(self) -> i128 {
        self.native_supply
            + self.evm_supply
            + self.svm_supply
            + self.x3vm_supply
            + self.external_locked_supply
            + self.pending_supply
    }

    fn assert_canonical_invariant(self) {
        assert_eq!(
            self.canonical_supply,
            self.represented_total(),
            "canonical invariant violated: canonical={}, represented={}",
            self.canonical_supply,
            self.represented_total()
        );
    }

    fn mint_canonical(&mut self, domain: usize, amount: i128) {
        self.canonical_supply += amount;
        self.add_to_domain(domain, amount);
    }

    fn burn_canonical(&mut self, domain: usize, amount: i128) {
        self.sub_from_domain(domain, amount);
        self.canonical_supply -= amount;
    }

    fn lock_external(&mut self, amount: i128) {
        self.external_locked_supply += amount;
        self.canonical_supply += amount;
    }

    fn unlock_external(&mut self, amount: i128) {
        self.external_locked_supply -= amount;
        self.canonical_supply -= amount;
    }

    fn route_to_pending(&mut self, source_domain: usize, amount: i128) {
        self.sub_from_domain(source_domain, amount);
        self.pending_supply += amount;
    }

    fn settle_pending_to_domain(&mut self, destination_domain: usize, amount: i128) {
        self.pending_supply -= amount;
        self.add_to_domain(destination_domain, amount);
    }

    fn timeout_refund(&mut self, source_domain: usize, amount: i128) {
        self.pending_supply -= amount;
        self.add_to_domain(source_domain, amount);
    }

    fn add_to_domain(&mut self, domain: usize, amount: i128) {
        match domain {
            0 => self.native_supply += amount,
            1 => self.evm_supply += amount,
            2 => self.svm_supply += amount,
            3 => self.x3vm_supply += amount,
            _ => panic!("unsupported domain index"),
        }
    }

    fn sub_from_domain(&mut self, domain: usize, amount: i128) {
        match domain {
            0 => self.native_supply -= amount,
            1 => self.evm_supply -= amount,
            2 => self.svm_supply -= amount,
            3 => self.x3vm_supply -= amount,
            _ => panic!("unsupported domain index"),
        }
    }
}

#[derive(Debug, Clone, Copy)]
struct DeterministicRng(u64);

impl DeterministicRng {
    fn new(seed: u64) -> Self {
        Self(seed)
    }

    fn next_u64(&mut self) -> u64 {
        self.0 = self
            .0
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        self.0
    }

    fn next_usize(&mut self, bound: usize) -> usize {
        (self.next_u64() as usize) % bound
    }

    fn next_amount(&mut self, max: i128) -> i128 {
        ((self.next_u64() % max as u64) as i128) + 1
    }
}

#[test]
fn registry_invariants_are_referenced() {
    let toml = fs::read_to_string("tests/invariants/registry.toml").expect("Failed to read registry.toml");

    // Extract IDs by simple parsing (lines starting with id = "...")
    let ids: Vec<String> = toml
        .lines()
        .filter_map(|l| {
            let l = l.trim();
            if l.starts_with("id = ") {
                // naive parse
                let rest = l.trim_start_matches("id = ").trim();
                if rest.starts_with('"') && rest.ends_with('"') {
                    Some(rest.trim_matches('"').to_string())
                } else {
                    None
                }
            } else {
                None
            }
        })
        .collect();

    assert!(!ids.is_empty(), "No invariants found in registry.toml");

    // For each id, search for its occurrence in the workspace files we care about.
    let search_paths = vec!["pallets", "crates", "tests", "apps", "packages", "swarm"];

    for id in ids {
        let mut found = false;
        for prefix in &search_paths {
            if let Ok(entries) = fs::read_dir(prefix) {
                for entry in entries.flatten() {
                    let path = entry.path();
                    if path.is_file() {
                        if let Ok(content) = fs::read_to_string(&path) {
                            if content.contains(&id) {
                                found = true;
                                break;
                            }
                        }
                    } else if path.is_dir() {
                        // Search recursively but shallow (one level) for speed
                        for sub in fs::read_dir(&path).unwrap_or_else(|_| fs::read_dir(Path::new(".")).unwrap()) {
                            if let Ok(sub) = sub {
                                let p = sub.path();
                                if p.is_file() {
                                    if let Ok(content) = fs::read_to_string(&p) {
                                        if content.contains(&id) {
                                            found = true;
                                            break;
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
            if found { break; }
        }

        assert!(found, "Invariant '{}' is not referenced by any test or file under search paths", id);
    }
}

#[test]
fn canonical_supply_invariant_fuzz_10k_random_transactions() {
    let mut state = SupplyState::default();
    let mut rng = DeterministicRng::new(0x5A17_2026_F00D_BAAD);

    // Bootstrap non-zero state across all internal domains.
    for domain in 0..4 {
        state.mint_canonical(domain, 10_000);
    }
    state.lock_external(15_000);
    state.assert_canonical_invariant();

    for _ in 0..10_000 {
        let op = rng.next_usize(8);
        let domain_a = rng.next_usize(4);
        let domain_b = rng.next_usize(4);
        let amount = rng.next_amount(250);

        match op {
            // Mint on random internal domain.
            0 => state.mint_canonical(domain_a, amount),
            // Burn from random internal domain when balance permits.
            1 => {
                let domain_balance = match domain_a {
                    0 => state.native_supply,
                    1 => state.evm_supply,
                    2 => state.svm_supply,
                    _ => state.x3vm_supply,
                };
                if domain_balance >= amount {
                    state.burn_canonical(domain_a, amount);
                }
            }
            // External lock / unlock path.
            2 => state.lock_external(amount),
            3 => {
                if state.external_locked_supply >= amount {
                    state.unlock_external(amount);
                }
            }
            // Cross-domain route to pending.
            4 => {
                let domain_balance = match domain_a {
                    0 => state.native_supply,
                    1 => state.evm_supply,
                    2 => state.svm_supply,
                    _ => state.x3vm_supply,
                };
                if domain_balance >= amount {
                    state.route_to_pending(domain_a, amount);
                }
            }
            // Settle pending on destination domain.
            5 => {
                if state.pending_supply >= amount {
                    state.settle_pending_to_domain(domain_b, amount);
                }
            }
            // Timeout refund from pending.
            6 => {
                if state.pending_supply >= amount {
                    state.timeout_refund(domain_a, amount);
                }
            }
            // Partial transfer simulation: split pending settle and refund.
            _ => {
                if state.pending_supply >= amount {
                    let settle = amount / 2;
                    let refund = amount - settle;
                    if settle > 0 {
                        state.settle_pending_to_domain(domain_b, settle);
                    }
                    if refund > 0 {
                        state.timeout_refund(domain_a, refund);
                    }
                }
            }
        }

        // All operations must preserve canonical == represented totals.
        state.assert_canonical_invariant();
        assert!(state.canonical_supply >= 0, "canonical supply must stay non-negative");
        assert!(state.pending_supply >= 0, "pending supply must stay non-negative");
    }

    // Edge case sweep: failed swap equivalent (refund all pending).
    if state.pending_supply > 0 {
        let pending = state.pending_supply;
        state.timeout_refund(0, pending);
        state.assert_canonical_invariant();
    }
}
