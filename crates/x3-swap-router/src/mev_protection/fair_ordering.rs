//! Deterministic commit–reveal ordering lane — X3-MEV-008 fair transaction ordering.
//!
//! # What this is
//!
//! A window-bounded commit–reveal lane. A participant that wants its swap
//! ordered by the *contents of a fixed window* — and never by the order in
//! which transactions happened to arrive — first commits
//!
//! ```text
//! commit_hash = BLAKE2b-256("X3:FAIR_ORDER:V1" ‖ plaintext ‖ nonce ‖ sender)
//! ```
//!
//! inside an [`OrderingWindow`] while posting a bond, and later reveals
//! `(plaintext, nonce)`. Once the window has closed, [`CommitRevealLane::settle`]
//! returns the canonical order of the revealed transactions.
//!
//! The canonical order is a total order over the revealed commitments' *order
//! keys*, so it is fixed by the set of commitments in the window. It cannot be
//! changed by anything that lands later, by the arrival order inside the window,
//! or by whoever submits the batch that settles it. Nothing in this module reads
//! a clock, and no `HashMap` iteration decides a sequence: every collection that
//! participates in ordering is a `BTreeMap`, whose iteration order is the key
//! order.
//!
//! # What this is not
//!
//! * **It is not grinding-resistant on its own.** With no beacon installed the
//!   order key *is* the commit hash, so a participant that can try many nonces
//!   can choose where its *own* commitment lands relative to commitments it can
//!   already see. What such a participant cannot do is move, drop or reorder
//!   anyone else's commitment: those positions follow from hashes it does not
//!   control, and the sort is recomputed from the closed set. One commitment per
//!   sender per window is enforced, but that is one per *address* — Sybil
//!   addresses are not prevented here. [`CommitRevealLane::install_beacon`]
//!   closes the placement hole by folding a value that may only be installed
//!   *after* the window has closed into the order key; the unpredictability of
//!   that beacon is an off-chain assumption this module neither verifies nor
//!   claims.
//! * **It does not execute anything.** It produces an order over opaque bytes.
//!   Whether the ordered payloads are admissible is the router's business.
//! * **It does not move the bond.** Unrevealed commitments are reported in
//!   [`WindowSettlement::unrevealed`] with the amount that should be forfeited;
//!   applying that is the caller's business. A lane that claimed to have slashed
//!   would be claiming an effect it cannot have.

use serde::{Deserialize, Serialize};
use sp_core::hashing::blake2_256;
use sp_core::{H160, H256};
use sp_std::collections::btree_map::BTreeMap;
use sp_std::vec::Vec;

/// Domain tag mixed into every commitment hash.
///
/// Versioned with the lane's semantics: changing what a commitment means means
/// changing this tag, so a hash produced under the old rules can never be
/// accepted under the new ones.
pub const COMMITMENT_DOMAIN: &[u8] = b"X3:FAIR_ORDER:V1";

/// Largest plaintext a single reveal may carry.
///
/// A reveal stores the payload it reveals, so an unbounded one is an unbounded
/// allocation driven by whoever reveals. 64 KiB is the same order as a large
/// multi-hop batch payload and small enough that a window's worth of them fits
/// in memory.
pub const MAX_PLAINTEXT_BYTES: usize = 64 * 1024;

/// Largest number of commitments one window accepts.
///
/// A lane is held in memory for the life of its window, so an unbounded
/// participant set is an unbounded allocation driven by whoever commits. The
/// bound also makes an entry's position representable: at most this many
/// entries exist, and this value fits in a `u32`.
pub const MAX_COMMITMENTS: usize = 65_536;

/// The commitment hash a participant must publish, and the reveal must re-derive.
///
/// This is the single constructor for that hash: the committer and the lane both
/// come through here, so the thing committed and the thing checked cannot drift
/// apart. The sender is bound in last so that a commitment lifted from the
/// mempool cannot be revealed by anyone else.
pub fn commitment_hash(sender: H160, plaintext: &[u8], nonce: &[u8; 32]) -> H256 {
    let mut buffer = Vec::with_capacity(COMMITMENT_DOMAIN.len() + plaintext.len() + 32 + 20);
    buffer.extend_from_slice(COMMITMENT_DOMAIN);
    buffer.extend_from_slice(plaintext);
    buffer.extend_from_slice(nonce);
    buffer.extend_from_slice(sender.as_bytes());
    H256::from(blake2_256(&buffer))
}

/// The key a commitment is ordered by.
///
/// Exposed so an observer can recompute the canonical order of a
/// [`WindowSettlement`] from the settlement alone rather than trusting the
/// settle-time computation. `None` means no beacon was installed, in which case
/// the key is the commit hash itself.
pub fn order_key(beacon: Option<H256>, commit_hash: &H256) -> H256 {
    match beacon {
        None => *commit_hash,
        Some(beacon) => {
            let mut buffer = [0u8; 64];
            buffer[..32].copy_from_slice(beacon.as_bytes());
            buffer[32..].copy_from_slice(commit_hash.as_bytes());
            H256::from(blake2_256(&buffer))
        }
    }
}

/// The block range a commit and its reveal must both land inside.
///
/// Both ends are inclusive. The close block is also the deadline: a reveal that
/// arrives after it is refused rather than included, because including it would
/// let a participant choose whether to reveal based on what everyone else
/// revealed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct OrderingWindow {
    pub open_block: u64,
    pub close_block: u64,
}

impl OrderingWindow {
    /// Build a window, refusing one with no block in it.
    pub fn new(open_block: u64, close_block: u64) -> Result<Self, FairOrderError> {
        if open_block > close_block {
            return Err(FairOrderError::InvertedWindow {
                open_block,
                close_block,
            });
        }
        Ok(Self {
            open_block,
            close_block,
        })
    }

    /// Whether `block` is inside the window.
    pub fn contains(&self, block: u64) -> bool {
        block >= self.open_block && block <= self.close_block
    }
}

/// A commitment as the lane records it. Carries no plaintext: see [`Reveal`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Commitment {
    pub commit_hash: H256,
    pub sender: H160,
    pub bond: u128,
    pub committed_at_block: u64,
}

/// A reveal the lane accepted. The plaintext is kept because a settlement that
/// ordered transactions without being able to hand them on would order nothing.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Reveal {
    pub commit_hash: H256,
    pub sender: H160,
    pub plaintext: Vec<u8>,
    pub nonce: [u8; 32],
    pub revealed_at_block: u64,
}

/// One transaction in the canonical order.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OrderedTransaction {
    /// Position in the canonical sequence, starting at zero.
    pub position: u32,
    /// The key this transaction was ordered by; recomputable via [`order_key`].
    pub order_key: H256,
    pub commit_hash: H256,
    pub sender: H160,
    pub plaintext: Vec<u8>,
    pub nonce: [u8; 32],
    pub bond: u128,
}

/// The canonical order for one closed window, and what did not make it in.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WindowSettlement {
    pub window: OrderingWindow,
    /// The beacon the order keys were computed with, or `None`.
    pub beacon: Option<H256>,
    /// Revealed transactions in canonical order.
    pub order: Vec<OrderedTransaction>,
    /// Commitments that never revealed, in commit-hash order. Their bonds are the
    /// caller's to forfeit; this lane only reports them.
    pub unrevealed: Vec<Commitment>,
}

impl WindowSettlement {
    /// Total bond reported as forfeitable for commitments that never revealed.
    pub fn forfeitable_bond(&self) -> u128 {
        self.unrevealed
            .iter()
            .map(|commitment| commitment.bond)
            .fold(0u128, u128::saturating_add)
    }

    /// Recompute the canonical order from the settlement alone.
    ///
    /// Sorted by `(order_key, commit_hash)`; the commit hash is a tie-break so
    /// that the order is total even in the (cryptographically improbable) case
    /// of two commitments sharing an order key, and so a verifier never has to
    /// reproduce a "these are equal" judgement to check a sequence.
    pub fn canonical_order(&self) -> Vec<H256> {
        let mut keys: Vec<(H256, H256)> = self
            .order
            .iter()
            .map(|entry| {
                (
                    order_key(self.beacon, &entry.commit_hash),
                    entry.commit_hash,
                )
            })
            .collect();
        keys.sort();
        keys.into_iter()
            .map(|(_, commit_hash)| commit_hash)
            .collect()
    }
}

/// A fail-closed refusal from the ordering lane.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FairOrderError {
    InvertedWindow { open_block: u64, close_block: u64 },
    CommitBeforeWindow { current: u64, open_block: u64 },
    CommitAfterWindowClosed { current: u64, close_block: u64 },
    RevealBeforeWindow { current: u64, open_block: u64 },
    RevealAfterWindowClosed { current: u64, close_block: u64 },
    BondBelowMinimum { provided: u128, required: u128 },
    DuplicateCommit { sender: H160 },
    DuplicateCommitHash { commit_hash: H256 },
    WindowFull { capacity: usize },
    RevealWithoutCommit { commit_hash: H256 },
    RevealSenderMismatch { committed: H160, revealed: H160 },
    RevealHashMismatch { commit_hash: H256 },
    DuplicateReveal { commit_hash: H256 },
    WindowStillOpen { current: u64, close_block: u64 },
    WindowSettled,
    BeaconWhileWindowOpen { current: u64, close_block: u64 },
    DuplicateBeacon,
    PlaintextTooLarge { len: usize, max: usize },
}

impl core::fmt::Display for FairOrderError {
    fn fmt(&self, f: &mut core::fmt::Formatter) -> core::fmt::Result {
        match self {
            FairOrderError::InvertedWindow {
                open_block,
                close_block,
            } => write!(
                f,
                "ordering window opens at block {open_block} and closes at block {close_block}, so it \
                 contains nothing"
            ),
            FairOrderError::CommitBeforeWindow { current, open_block } => write!(
                f,
                "commit at block {current} is before the ordering window opens at block {open_block}"
            ),
            FairOrderError::CommitAfterWindowClosed { current, close_block } => write!(
                f,
                "commit at block {current} is after the ordering window closed at block {close_block}"
            ),
            FairOrderError::RevealBeforeWindow { current, open_block } => write!(
                f,
                "reveal at block {current} is before the ordering window opens at block {open_block}"
            ),
            FairOrderError::RevealAfterWindowClosed { current, close_block } => write!(
                f,
                "reveal at block {current} is after the ordering window closed at block {close_block}"
            ),
            FairOrderError::BondBelowMinimum { provided, required } => {
                write!(f, "bond {provided} is below the lane minimum {required}")
            }
            FairOrderError::DuplicateCommit { sender } => write!(
                f,
                "sender {sender:?} already committed in this window; one sender, one commitment"
            ),
            FairOrderError::DuplicateCommitHash { commit_hash } => write!(
                f,
                "commit hash {commit_hash:?} was already committed in this window"
            ),
            FairOrderError::WindowFull { capacity } => write!(
                f,
                "this ordering window already holds its maximum of {capacity} commitments"
            ),
            FairOrderError::RevealWithoutCommit { commit_hash } => write!(
                f,
                "reveal names commit hash {commit_hash:?}, which was never committed in this window"
            ),
            FairOrderError::RevealSenderMismatch { committed, revealed } => write!(
                f,
                "commitment belongs to sender {committed:?} but the reveal came from {revealed:?}"
            ),
            FairOrderError::RevealHashMismatch { commit_hash } => write!(
                f,
                "revealed plaintext, nonce and sender do not hash to the committed hash {commit_hash:?}"
            ),
            FairOrderError::DuplicateReveal { commit_hash } => {
                write!(f, "commit hash {commit_hash:?} was already revealed")
            }
            FairOrderError::WindowStillOpen { current, close_block } => write!(
                f,
                "cannot settle at block {current}: the ordering window is open until block {close_block}"
            ),
            FairOrderError::WindowSettled => {
                write!(f, "this ordering window has already been settled")
            }
            FairOrderError::BeaconWhileWindowOpen { current, close_block } => write!(
                f,
                "cannot install an ordering beacon at block {current}: participants can still commit \
                 until block {close_block}, so they could choose their hash against it"
            ),
            FairOrderError::DuplicateBeacon => {
                write!(f, "an ordering beacon is already installed for this window")
            }
            FairOrderError::PlaintextTooLarge { len, max } => {
                write!(f, "revealed plaintext is {len} bytes, above the {max}-byte limit")
            }
        }
    }
}

/// A window-bounded commit–reveal ordering lane.
///
/// One lane is one window. Every mutation is validated against the window and
/// against the lane's own state, and a refusal leaves the lane unchanged — a
/// rejected reveal cannot be used to probe the lane or to partially apply.
pub struct CommitRevealLane {
    window: OrderingWindow,
    minimum_bond: u128,
    /// Commitments by commit hash. `BTreeMap` so iteration is key order, not
    /// insertion order: the canonical sequence must not depend on arrival.
    commits: BTreeMap<H256, Commitment>,
    /// One commitment per sender, so a single participant cannot crowd the
    /// window with copies of itself.
    commit_by_sender: BTreeMap<H160, H256>,
    reveals: BTreeMap<H256, Reveal>,
    beacon: Option<H256>,
    settled: bool,
}

impl CommitRevealLane {
    /// Open a lane for `window`, requiring every commitment to post at least
    /// `minimum_bond`.
    pub fn new(window: OrderingWindow, minimum_bond: u128) -> Self {
        Self {
            window,
            minimum_bond,
            commits: BTreeMap::new(),
            commit_by_sender: BTreeMap::new(),
            reveals: BTreeMap::new(),
            beacon: None,
            settled: false,
        }
    }

    pub fn window(&self) -> OrderingWindow {
        self.window
    }

    pub fn minimum_bond(&self) -> u128 {
        self.minimum_bond
    }

    pub fn beacon(&self) -> Option<H256> {
        self.beacon
    }

    pub fn is_settled(&self) -> bool {
        self.settled
    }

    pub fn commitment_count(&self) -> usize {
        self.commits.len()
    }

    pub fn reveal_count(&self) -> usize {
        self.reveals.len()
    }

    /// Look up a commitment by its hash.
    pub fn commitment(&self, commit_hash: &H256) -> Option<&Commitment> {
        self.commits.get(commit_hash)
    }

    /// Record a commitment.
    ///
    /// `commit_hash` must be [`commitment_hash`]`(sender, plaintext, nonce)`; the
    /// lane never sees the plaintext at this point, which is the whole point.
    pub fn commit(
        &mut self,
        sender: H160,
        commit_hash: H256,
        bond: u128,
        current_block: u64,
    ) -> Result<(), FairOrderError> {
        if self.settled {
            return Err(FairOrderError::WindowSettled);
        }
        if current_block < self.window.open_block {
            return Err(FairOrderError::CommitBeforeWindow {
                current: current_block,
                open_block: self.window.open_block,
            });
        }
        if current_block > self.window.close_block {
            return Err(FairOrderError::CommitAfterWindowClosed {
                current: current_block,
                close_block: self.window.close_block,
            });
        }
        if bond < self.minimum_bond {
            return Err(FairOrderError::BondBelowMinimum {
                provided: bond,
                required: self.minimum_bond,
            });
        }
        if self.commits.len() >= MAX_COMMITMENTS {
            return Err(FairOrderError::WindowFull {
                capacity: MAX_COMMITMENTS,
            });
        }
        if self.commit_by_sender.contains_key(&sender) {
            return Err(FairOrderError::DuplicateCommit { sender });
        }
        if self.commits.contains_key(&commit_hash) {
            return Err(FairOrderError::DuplicateCommitHash { commit_hash });
        }

        self.commit_by_sender.insert(sender, commit_hash);
        self.commits.insert(
            commit_hash,
            Commitment {
                commit_hash,
                sender,
                bond,
                committed_at_block: current_block,
            },
        );
        Ok(())
    }

    /// Reveal a commitment.
    ///
    /// Checked in a fixed order — size, window, settled, duplicate, existence,
    /// sender, hash — so a caller can tell *which* rule refused it rather than
    /// only that something did. Refusing is the only outcome of a failed check;
    /// nothing is recorded on the way out.
    pub fn reveal(
        &mut self,
        sender: H160,
        commit_hash: H256,
        plaintext: &[u8],
        nonce: &[u8; 32],
        current_block: u64,
    ) -> Result<(), FairOrderError> {
        if plaintext.len() > MAX_PLAINTEXT_BYTES {
            return Err(FairOrderError::PlaintextTooLarge {
                len: plaintext.len(),
                max: MAX_PLAINTEXT_BYTES,
            });
        }
        if current_block < self.window.open_block {
            return Err(FairOrderError::RevealBeforeWindow {
                current: current_block,
                open_block: self.window.open_block,
            });
        }
        if current_block > self.window.close_block {
            return Err(FairOrderError::RevealAfterWindowClosed {
                current: current_block,
                close_block: self.window.close_block,
            });
        }
        if self.settled {
            return Err(FairOrderError::WindowSettled);
        }
        if self.reveals.contains_key(&commit_hash) {
            return Err(FairOrderError::DuplicateReveal { commit_hash });
        }
        let commitment = match self.commits.get(&commit_hash) {
            Some(commitment) => commitment.clone(),
            None => return Err(FairOrderError::RevealWithoutCommit { commit_hash }),
        };
        if commitment.sender != sender {
            return Err(FairOrderError::RevealSenderMismatch {
                committed: commitment.sender,
                revealed: sender,
            });
        }
        if commitment_hash(sender, plaintext, nonce) != commit_hash {
            return Err(FairOrderError::RevealHashMismatch { commit_hash });
        }

        self.reveals.insert(
            commit_hash,
            Reveal {
                commit_hash,
                sender,
                plaintext: plaintext.to_vec(),
                nonce: *nonce,
                revealed_at_block: current_block,
            },
        );
        Ok(())
    }

    /// Install the per-window ordering beacon.
    ///
    /// Only accepted once the window has closed, so no participant can choose a
    /// commit hash with the beacon already known. That is the *only* thing this
    /// method enforces, and it is not the whole property: whoever calls it can
    /// grind the beacon and thereby bias the whole order, so the beacon has to
    /// come from a source the caller cannot choose — a future block hash, a
    /// verifiable random function, a threshold signature. Provenance and
    /// unpredictability are outside this module, which neither verifies nor
    /// claims them.
    pub fn install_beacon(
        &mut self,
        beacon: H256,
        current_block: u64,
    ) -> Result<(), FairOrderError> {
        if self.settled {
            return Err(FairOrderError::WindowSettled);
        }
        if current_block <= self.window.close_block {
            return Err(FairOrderError::BeaconWhileWindowOpen {
                current: current_block,
                close_block: self.window.close_block,
            });
        }
        if self.beacon.is_some() {
            return Err(FairOrderError::DuplicateBeacon);
        }
        self.beacon = Some(beacon);
        Ok(())
    }

    /// Close the window and produce the canonical order.
    ///
    /// Refused while the window is still open: settling early would publish an
    /// order a later reveal could invalidate, and would let the settler choose the
    /// moment at which the participant set is frozen. After a successful settle
    /// the lane is spent — a second settle is refused rather than recomputed, so
    /// there is exactly one order per window.
    pub fn settle(&mut self, current_block: u64) -> Result<WindowSettlement, FairOrderError> {
        if self.settled {
            return Err(FairOrderError::WindowSettled);
        }
        if current_block <= self.window.close_block {
            return Err(FairOrderError::WindowStillOpen {
                current: current_block,
                close_block: self.window.close_block,
            });
        }

        let mut entries: Vec<(H256, H256)> = self
            .reveals
            .keys()
            .map(|commit_hash| (order_key(self.beacon, commit_hash), *commit_hash))
            .collect();
        entries.sort();

        let mut order = Vec::with_capacity(entries.len());
        for (position, (key, commit_hash)) in entries.into_iter().enumerate() {
            // Both lookups hold by the lane's own insert discipline: a reveal is
            // only ever recorded against a commitment already in `commits`, and
            // neither map ever drops an entry. The position conversion holds
            // because `commit` refuses the (MAX_COMMITMENTS + 1)th commitment and
            // MAX_COMMITMENTS fits in a `u32`.
            let reveal = self
                .reveals
                .get(&commit_hash)
                .expect("an order entry names a commit hash the lane already revealed");
            let commitment = self
                .commits
                .get(&commit_hash)
                .expect("a reveal is only accepted against a commitment the lane recorded");
            order.push(OrderedTransaction {
                position: u32::try_from(position)
                    .expect("the window holds at most MAX_COMMITMENTS entries, which fits in u32"),
                order_key: key,
                commit_hash,
                sender: reveal.sender,
                plaintext: reveal.plaintext.clone(),
                nonce: reveal.nonce,
                bond: commitment.bond,
            });
        }

        let unrevealed: Vec<Commitment> = self
            .commits
            .values()
            .filter(|commitment| !self.reveals.contains_key(&commitment.commit_hash))
            .cloned()
            .collect();

        self.settled = true;
        Ok(WindowSettlement {
            window: self.window,
            beacon: self.beacon,
            order,
            unrevealed,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const MIN_BOND: u128 = 1_000;
    const OPEN: u64 = 100;
    const CLOSE: u64 = 110;

    fn open_lane() -> CommitRevealLane {
        CommitRevealLane::new(
            OrderingWindow::new(OPEN, CLOSE).expect("100..=110 is a window"),
            MIN_BOND,
        )
    }

    /// Deterministic participant fixture: sender `i + 1`, nonce `i + 1`, payload
    /// `b"swap-<i+1>"`.
    struct Participant {
        sender: H160,
        nonce: [u8; 32],
        plaintext: Vec<u8>,
    }

    impl Participant {
        fn commit_hash(&self) -> H256 {
            commitment_hash(self.sender, &self.plaintext, &self.nonce)
        }
    }

    fn participants(count: u8) -> Vec<Participant> {
        (0..count)
            .map(|index| {
                let byte = index + 1;
                let mut plaintext = b"swap-".to_vec();
                plaintext.push(byte);
                Participant {
                    sender: H160::repeat_byte(byte),
                    nonce: [byte; 32],
                    plaintext,
                }
            })
            .collect()
    }

    fn commit_all(
        lane: &mut CommitRevealLane,
        people: &[Participant],
        order: &[usize],
        block: u64,
    ) {
        for index in order {
            let person = &people[*index];
            lane.commit(person.sender, person.commit_hash(), MIN_BOND, block)
                .expect("a well-formed commitment inside the window is accepted");
        }
    }

    fn reveal_all(
        lane: &mut CommitRevealLane,
        people: &[Participant],
        order: &[usize],
        block: u64,
    ) {
        for index in order {
            let person = &people[*index];
            lane.reveal(
                person.sender,
                person.commit_hash(),
                &person.plaintext,
                &person.nonce,
                block,
            )
            .expect("a matching reveal inside the window is accepted");
        }
    }

    fn hashes(settlement: &WindowSettlement) -> Vec<H256> {
        settlement
            .order
            .iter()
            .map(|entry| entry.commit_hash)
            .collect()
    }

    #[test]
    fn a_closed_window_settles_in_canonical_key_order() {
        let people = participants(3);
        // Arrive out of hash order on purpose.
        let arrival = [2usize, 0, 1];
        let mut lane = open_lane();
        commit_all(&mut lane, &people, &arrival, OPEN);
        reveal_all(&mut lane, &people, &arrival, OPEN + 1);

        let settlement = lane.settle(CLOSE + 1).expect("a closed window settles");

        let mut expected: Vec<H256> = people.iter().map(Participant::commit_hash).collect();
        expected.sort();
        assert_eq!(
            hashes(&settlement),
            expected,
            "the canonical order is the key order"
        );
        assert_eq!(
            settlement.canonical_order(),
            expected,
            "and a verifier recomputes it"
        );
        assert!(settlement.unrevealed.is_empty());

        for (index, entry) in settlement.order.iter().enumerate() {
            assert_eq!(
                entry.position,
                u32::try_from(index).expect("three entries fit in u32")
            );
            assert_eq!(entry.bond, MIN_BOND);
            let person = people
                .iter()
                .find(|person| person.commit_hash() == entry.commit_hash)
                .expect("the order only names participants that committed");
            assert_eq!(entry.sender, person.sender);
            assert_eq!(
                entry.plaintext, person.plaintext,
                "the order carries the revealed payload"
            );
            assert_eq!(entry.nonce, person.nonce);
        }
    }

    fn settle_in_order(commit_order: &[usize], reveal_order: &[usize]) -> Vec<H256> {
        let people = participants(4);
        let mut lane = open_lane();
        commit_all(&mut lane, &people, commit_order, OPEN);
        reveal_all(&mut lane, &people, reveal_order, OPEN + 2);
        hashes(&lane.settle(CLOSE + 1).expect("a closed window settles"))
    }

    #[test]
    fn the_sequence_does_not_depend_on_arrival_order() {
        let baseline = settle_in_order(&[0, 1, 2, 3], &[0, 1, 2, 3]);
        let commit_orders: [&[usize]; 4] =
            [&[0, 1, 2, 3], &[3, 2, 1, 0], &[1, 3, 0, 2], &[2, 0, 3, 1]];
        let reveal_orders: [&[usize]; 3] = [&[0, 1, 2, 3], &[3, 2, 1, 0], &[2, 3, 1, 0]];
        for commit_order in commit_orders {
            for reveal_order in reveal_orders {
                assert_eq!(
                    settle_in_order(commit_order, reveal_order),
                    baseline,
                    "commits {commit_order:?} revealed {reveal_order:?} settle to the same sequence"
                );
            }
        }
    }

    #[test]
    fn the_sequence_is_the_key_order_and_not_the_arrival_order() {
        let people = participants(4);
        let mut sorted: Vec<H256> = people.iter().map(Participant::commit_hash).collect();
        sorted.sort();
        // Commit in the reverse of the canonical order, so anything that settled by
        // arrival would produce a visibly different sequence.
        let arrival: Vec<usize> = sorted
            .iter()
            .rev()
            .map(|hash| {
                people
                    .iter()
                    .position(|person| person.commit_hash() == *hash)
                    .expect("every sorted hash belongs to a participant")
            })
            .collect();

        let settled = settle_in_order(&arrival, &arrival);
        let arrived: Vec<H256> = arrival
            .iter()
            .map(|index| people[*index].commit_hash())
            .collect();
        assert_ne!(
            settled, arrived,
            "arrival order must not be the canonical order"
        );
        assert_eq!(settled, sorted, "the canonical order is the key order");
    }

    #[test]
    fn the_reveal_block_within_the_window_does_not_change_the_sequence() {
        let people = participants(3);
        let settle_at = |block: u64| {
            let mut lane = open_lane();
            commit_all(&mut lane, &people, &[0, 1, 2], OPEN);
            reveal_all(&mut lane, &people, &[0, 1, 2], block);
            hashes(&lane.settle(CLOSE + 1).expect("a closed window settles"))
        };
        assert_eq!(
            settle_at(OPEN),
            settle_at(CLOSE),
            "nothing in the order may read a clock or an arrival block"
        );
    }

    #[test]
    fn reveals_landing_at_different_blocks_still_order_by_key() {
        let people = participants(3);
        let mut sorted: Vec<H256> = people.iter().map(Participant::commit_hash).collect();
        sorted.sort();
        // Commit and reveal one participant per block, in the reverse of the
        // canonical order: a lane that ordered by arrival block, or by the order a
        // reveal was accepted, would publish the reverse of the key order.
        let arrival: Vec<usize> = sorted
            .iter()
            .rev()
            .map(|hash| {
                people
                    .iter()
                    .position(|person| person.commit_hash() == *hash)
                    .expect("every sorted hash belongs to a participant")
            })
            .collect();

        let mut lane = open_lane();
        for (step, index) in arrival.iter().enumerate() {
            let block = OPEN + u64::try_from(step).expect("three steps fit in u64");
            let person = &people[*index];
            lane.commit(person.sender, person.commit_hash(), MIN_BOND, block)
                .expect("a commit inside the window is accepted");
        }
        for (step, index) in arrival.iter().enumerate() {
            let block = OPEN + 5 + u64::try_from(step).expect("three steps fit in u64");
            let person = &people[*index];
            lane.reveal(
                person.sender,
                person.commit_hash(),
                &person.plaintext,
                &person.nonce,
                block,
            )
            .expect("a reveal inside the window is accepted");
        }

        let settlement = lane.settle(CLOSE + 1).expect("a closed window settles");
        assert_eq!(
            hashes(&settlement),
            sorted,
            "the order is the key order, whatever block each commit and reveal landed in"
        );
        assert_ne!(
            hashes(&settlement),
            arrival
                .iter()
                .map(|index| people[*index].commit_hash())
                .collect::<Vec<_>>(),
            "arrival block must not decide the sequence"
        );
    }

    #[test]
    fn settling_before_the_window_closes_is_refused() {
        let mut lane = open_lane();
        assert_eq!(
            lane.settle(CLOSE)
                .expect_err("an open window cannot settle"),
            FairOrderError::WindowStillOpen {
                current: CLOSE,
                close_block: CLOSE
            }
        );
        assert!(
            !lane.is_settled(),
            "a refused settle must not spend the window"
        );
        assert!(lane.settle(CLOSE + 1).is_ok());
    }

    #[test]
    fn settling_twice_is_refused() {
        let mut lane = open_lane();
        lane.settle(CLOSE + 1).expect("the first settle wins");
        assert_eq!(
            lane.settle(CLOSE + 2)
                .expect_err("a settled window is spent"),
            FairOrderError::WindowSettled
        );
    }

    #[test]
    fn a_settled_window_accepts_no_further_commits_or_reveals() {
        let people = participants(1);
        let person = &people[0];
        let mut lane = open_lane();
        lane.commit(person.sender, person.commit_hash(), MIN_BOND, OPEN)
            .expect("the commitment is inside the window");
        lane.settle(CLOSE + 1).expect("a closed window settles");

        assert_eq!(
            lane.commit(person.sender, person.commit_hash(), MIN_BOND, OPEN)
                .expect_err("a settled window takes no commitments"),
            FairOrderError::WindowSettled
        );
        assert_eq!(
            lane.reveal(
                person.sender,
                person.commit_hash(),
                &person.plaintext,
                &person.nonce,
                OPEN
            )
            .expect_err("a settled window takes no reveals"),
            FairOrderError::WindowSettled
        );
    }

    #[test]
    fn a_reveal_without_a_commit_is_refused() {
        let people = participants(1);
        let person = &people[0];
        let mut lane = open_lane();
        assert_eq!(
            lane.reveal(
                person.sender,
                person.commit_hash(),
                &person.plaintext,
                &person.nonce,
                OPEN
            )
            .expect_err("a reveal needs a commitment"),
            FairOrderError::RevealWithoutCommit {
                commit_hash: person.commit_hash()
            }
        );
        assert_eq!(lane.reveal_count(), 0);
        assert_eq!(lane.commitment_count(), 0);
    }

    #[test]
    fn a_reveal_after_the_window_closes_is_refused_and_never_ordered() {
        let people = participants(2);
        let mut lane = open_lane();
        commit_all(&mut lane, &people, &[0, 1], OPEN);
        reveal_all(&mut lane, &people, &[0], CLOSE);

        let late = people[1].commit_hash();
        assert_eq!(
            lane.reveal(
                people[1].sender,
                late,
                &people[1].plaintext,
                &people[1].nonce,
                CLOSE + 1
            )
            .expect_err("a reveal after the deadline is refused"),
            FairOrderError::RevealAfterWindowClosed {
                current: CLOSE + 1,
                close_block: CLOSE
            }
        );
        assert_eq!(lane.reveal_count(), 1, "the late reveal was not recorded");

        let settlement = lane.settle(CLOSE + 1).expect("a closed window settles");
        assert_eq!(settlement.order.len(), 1);
        assert_eq!(settlement.unrevealed.len(), 1);
        assert_eq!(settlement.unrevealed[0].commit_hash, late);
        assert_eq!(settlement.forfeitable_bond(), MIN_BOND);
    }

    #[test]
    fn a_reveal_before_the_window_opens_is_refused() {
        let people = participants(1);
        let person = &people[0];
        let mut lane = open_lane();
        lane.commit(person.sender, person.commit_hash(), MIN_BOND, OPEN)
            .expect("the commitment is inside the window");
        assert_eq!(
            lane.reveal(
                person.sender,
                person.commit_hash(),
                &person.plaintext,
                &person.nonce,
                OPEN - 1
            )
            .expect_err("a reveal before the window opens is refused"),
            FairOrderError::RevealBeforeWindow {
                current: OPEN - 1,
                open_block: OPEN
            }
        );
        assert_eq!(lane.reveal_count(), 0);
    }

    #[test]
    fn a_reveal_that_does_not_hash_to_its_commit_is_refused() {
        let people = participants(1);
        let person = &people[0];
        let commit_hash = person.commit_hash();
        let mut lane = open_lane();
        lane.commit(person.sender, commit_hash, MIN_BOND, OPEN)
            .expect("the commitment is inside the window");

        let substituted = b"send-everything-to-me".to_vec();
        assert_eq!(
            lane.reveal(
                person.sender,
                commit_hash,
                &substituted,
                &person.nonce,
                OPEN
            )
            .expect_err("a substituted payload is refused"),
            FairOrderError::RevealHashMismatch { commit_hash }
        );

        let mut other_nonce = person.nonce;
        other_nonce[0] ^= 0xFF;
        assert_eq!(
            lane.reveal(
                person.sender,
                commit_hash,
                &person.plaintext,
                &other_nonce,
                OPEN
            )
            .expect_err("a swapped nonce is refused"),
            FairOrderError::RevealHashMismatch { commit_hash }
        );
        assert_eq!(lane.reveal_count(), 0, "no refused reveal may be recorded");
    }

    #[test]
    fn a_second_reveal_of_the_same_commitment_is_refused() {
        let people = participants(1);
        let person = &people[0];
        let mut lane = open_lane();
        lane.commit(person.sender, person.commit_hash(), MIN_BOND, OPEN)
            .expect("the commitment is inside the window");
        lane.reveal(
            person.sender,
            person.commit_hash(),
            &person.plaintext,
            &person.nonce,
            OPEN,
        )
        .expect("the first reveal is accepted");

        assert_eq!(
            lane.reveal(
                person.sender,
                person.commit_hash(),
                &person.plaintext,
                &person.nonce,
                OPEN
            )
            .expect_err("a commitment reveals once"),
            FairOrderError::DuplicateReveal {
                commit_hash: person.commit_hash()
            }
        );
        assert_eq!(lane.reveal_count(), 1);
    }

    #[test]
    fn a_reveal_from_a_different_sender_is_refused() {
        let people = participants(2);
        let owner = &people[0];
        let stranger = &people[1];
        let mut lane = open_lane();
        lane.commit(owner.sender, owner.commit_hash(), MIN_BOND, OPEN)
            .expect("the commitment is inside the window");

        assert_eq!(
            lane.reveal(
                stranger.sender,
                owner.commit_hash(),
                &owner.plaintext,
                &owner.nonce,
                OPEN
            )
            .expect_err("only the committer may reveal"),
            FairOrderError::RevealSenderMismatch {
                committed: owner.sender,
                revealed: stranger.sender
            }
        );
        assert_eq!(lane.reveal_count(), 0);
    }

    #[test]
    fn an_attacker_cannot_substitute_its_payload_for_a_victims_commitment() {
        let people = participants(2);
        let victim = &people[0];
        let attacker = &people[1];
        let mut lane = open_lane();
        lane.commit(victim.sender, victim.commit_hash(), MIN_BOND, OPEN)
            .expect("the victim's commitment is inside the window");
        lane.commit(attacker.sender, attacker.commit_hash(), MIN_BOND, OPEN)
            .expect("the attacker's own commitment is inside the window");

        let mismatch = FairOrderError::RevealSenderMismatch {
            committed: victim.sender,
            revealed: attacker.sender,
        };
        // Revealing the victim's commitment with the attacker's own payload, and
        // again with the victim's payload, are both refusals: the sender is bound
        // into the commitment hash, so the payload alone cannot be swapped in.
        assert_eq!(
            lane.reveal(
                attacker.sender,
                victim.commit_hash(),
                &attacker.plaintext,
                &attacker.nonce,
                OPEN
            )
            .expect_err("a lift cannot be revealed by another sender"),
            mismatch.clone()
        );
        assert_eq!(
            lane.reveal(
                attacker.sender,
                victim.commit_hash(),
                &victim.plaintext,
                &victim.nonce,
                OPEN
            )
            .expect_err("knowing the payload does not transfer the commitment"),
            mismatch
        );
        assert_eq!(lane.reveal_count(), 0);
    }

    #[test]
    fn a_commitment_that_never_reveals_is_excluded_and_its_bond_reported() {
        let people = participants(3);
        let mut lane = open_lane();
        commit_all(&mut lane, &people, &[0, 1, 2], OPEN);
        reveal_all(&mut lane, &people, &[0, 2], OPEN + 1);

        let settlement = lane.settle(CLOSE + 1).expect("a closed window settles");
        assert_eq!(settlement.order.len(), 2);
        assert!(
            !settlement
                .order
                .iter()
                .any(|entry| entry.commit_hash == people[1].commit_hash()),
            "an unrevealed commitment must not be ordered"
        );
        assert_eq!(settlement.unrevealed.len(), 1);
        assert_eq!(settlement.unrevealed[0].sender, people[1].sender);
        assert_eq!(settlement.unrevealed[0].bond, MIN_BOND);
        assert_eq!(settlement.forfeitable_bond(), MIN_BOND);
    }

    #[test]
    fn a_commit_outside_the_window_is_refused() {
        let people = participants(1);
        let person = &people[0];
        let mut lane = open_lane();
        assert_eq!(
            lane.commit(person.sender, person.commit_hash(), MIN_BOND, OPEN - 1)
                .expect_err("a commit before the window opens is refused"),
            FairOrderError::CommitBeforeWindow {
                current: OPEN - 1,
                open_block: OPEN
            }
        );
        assert_eq!(
            lane.commit(person.sender, person.commit_hash(), MIN_BOND, CLOSE + 1)
                .expect_err("a commit after the deadline is refused"),
            FairOrderError::CommitAfterWindowClosed {
                current: CLOSE + 1,
                close_block: CLOSE
            }
        );
        assert_eq!(
            lane.commitment_count(),
            0,
            "a refused commit must not be recorded"
        );
        lane.commit(person.sender, person.commit_hash(), MIN_BOND, OPEN)
            .expect("the window is still usable after refusals");
    }

    #[test]
    fn a_bond_below_the_minimum_is_refused() {
        let people = participants(1);
        let person = &people[0];
        let mut lane = open_lane();
        assert_eq!(
            lane.commit(person.sender, person.commit_hash(), MIN_BOND - 1, OPEN)
                .expect_err("an underbonded commitment is refused"),
            FairOrderError::BondBelowMinimum {
                provided: MIN_BOND - 1,
                required: MIN_BOND
            }
        );
        assert_eq!(lane.commitment_count(), 0);
    }

    #[test]
    fn one_sender_may_not_commit_twice_in_a_window() {
        let people = participants(1);
        let person = &people[0];
        let mut lane = open_lane();
        lane.commit(person.sender, person.commit_hash(), MIN_BOND, OPEN)
            .expect("the first commitment is accepted");

        let second_hash = commitment_hash(person.sender, b"swap-again", &[0x99; 32]);
        assert_eq!(
            lane.commit(person.sender, second_hash, MIN_BOND, OPEN)
                .expect_err("one sender, one commitment"),
            FairOrderError::DuplicateCommit {
                sender: person.sender
            }
        );
        assert_eq!(lane.commitment_count(), 1);
    }

    #[test]
    fn the_same_commit_hash_from_two_senders_is_refused() {
        let people = participants(2);
        let mut lane = open_lane();
        lane.commit(people[0].sender, people[0].commit_hash(), MIN_BOND, OPEN)
            .expect("the first commitment is accepted");
        assert_eq!(
            lane.commit(people[1].sender, people[0].commit_hash(), MIN_BOND, OPEN)
                .expect_err("a commit hash names one commitment"),
            FairOrderError::DuplicateCommitHash {
                commit_hash: people[0].commit_hash()
            }
        );
        assert_eq!(lane.commitment_count(), 1);
    }

    #[test]
    fn an_inverted_window_is_refused() {
        assert_eq!(
            OrderingWindow::new(CLOSE, OPEN).expect_err("a window with no block in it is refused"),
            FairOrderError::InvertedWindow {
                open_block: CLOSE,
                close_block: OPEN
            }
        );
        let single = OrderingWindow::new(OPEN, OPEN).expect("a one-block window is a window");
        assert!(single.contains(OPEN));
        assert!(!single.contains(OPEN - 1));
        assert!(!single.contains(OPEN + 1));
    }

    #[test]
    fn a_beacon_installed_while_the_window_is_open_is_refused() {
        let mut lane = open_lane();
        assert_eq!(
            lane.install_beacon(H256::repeat_byte(7), CLOSE)
                .expect_err("a beacon cannot be chosen while participants can still commit"),
            FairOrderError::BeaconWhileWindowOpen {
                current: CLOSE,
                close_block: CLOSE
            }
        );
        assert_eq!(lane.beacon(), None);

        lane.install_beacon(H256::repeat_byte(7), CLOSE + 1)
            .expect("a closed window accepts the beacon");
        assert_eq!(lane.beacon(), Some(H256::repeat_byte(7)));
        assert_eq!(
            lane.install_beacon(H256::repeat_byte(8), CLOSE + 1)
                .expect_err("one beacon per window"),
            FairOrderError::DuplicateBeacon
        );
        assert_eq!(lane.beacon(), Some(H256::repeat_byte(7)));
    }

    #[test]
    fn a_beacon_reorders_and_the_order_stays_recomputable_from_the_settlement() {
        let people = participants(4);
        let build = |beacon: Option<H256>| -> WindowSettlement {
            let mut lane = open_lane();
            commit_all(&mut lane, &people, &[0, 1, 2, 3], OPEN);
            reveal_all(&mut lane, &people, &[0, 1, 2, 3], OPEN + 1);
            if let Some(beacon) = beacon {
                lane.install_beacon(beacon, CLOSE + 1)
                    .expect("a closed window accepts the beacon");
            }
            lane.settle(CLOSE + 2).expect("a closed window settles")
        };

        let without = build(None);
        // Pick a beacon that actually reorders this fixture, so the assertion is
        // about the beacon keying the order rather than about a coincidence. The
        // search is over a fixed sequence, so the test is deterministic.
        let beacon = (1u8..=64)
            .map(H256::repeat_byte)
            .find(|candidate| {
                build(Some(*candidate)).canonical_order() != without.canonical_order()
            })
            .expect("a four-element order has 24 permutations, so a reordering beacon exists");
        let with = build(Some(beacon));

        assert_eq!(with.beacon, Some(beacon));
        assert_ne!(
            with.canonical_order(),
            without.canonical_order(),
            "the installed beacon must be what the order is derived from"
        );
        let published: Vec<H256> = with.order.iter().map(|entry| entry.commit_hash).collect();
        assert_eq!(
            with.canonical_order(),
            published,
            "the published order recomputes"
        );
        for entry in &with.order {
            assert_eq!(entry.order_key, order_key(Some(beacon), &entry.commit_hash));
        }
    }

    #[test]
    fn an_oversized_reveal_is_refused() {
        let people = participants(1);
        let person = &people[0];
        let oversized = vec![0xAB; MAX_PLAINTEXT_BYTES + 1];
        // Commit to the oversized payload so that only the size rule can refuse it.
        let hash = commitment_hash(person.sender, &oversized, &person.nonce);
        let mut lane = open_lane();
        lane.commit(person.sender, hash, MIN_BOND, OPEN)
            .expect("the commitment is inside the window");
        assert_eq!(
            lane.reveal(person.sender, hash, &oversized, &person.nonce, OPEN)
                .expect_err("an unbounded reveal is refused"),
            FairOrderError::PlaintextTooLarge {
                len: MAX_PLAINTEXT_BYTES + 1,
                max: MAX_PLAINTEXT_BYTES
            }
        );
        assert_eq!(lane.reveal_count(), 0);
    }

    #[test]
    fn a_window_beyond_its_capacity_is_refused() {
        let mut lane = CommitRevealLane::new(
            OrderingWindow::new(OPEN, CLOSE).expect("100..=110 is a window"),
            0,
        );
        for index in 0..MAX_COMMITMENTS {
            let counter = u32::try_from(index).expect("the capacity fits in u32");
            let mut address = [0u8; 20];
            address[..4].copy_from_slice(&counter.to_le_bytes());
            let sender = H160::from(address);
            let mut nonce = [0u8; 32];
            nonce[..4].copy_from_slice(&counter.to_le_bytes());
            lane.commit(sender, commitment_hash(sender, b"payload", &nonce), 0, OPEN)
                .expect("a commitment inside the capacity is accepted");
        }
        assert_eq!(lane.commitment_count(), MAX_COMMITMENTS);

        let mut address = [0u8; 20];
        address[..4].copy_from_slice(&u32::MAX.to_le_bytes());
        let overflow_sender = H160::from(address);
        assert_eq!(
            lane.commit(
                overflow_sender,
                commitment_hash(overflow_sender, b"payload", &[0xFF; 32]),
                0,
                OPEN
            )
            .expect_err("a window has a hard capacity"),
            FairOrderError::WindowFull {
                capacity: MAX_COMMITMENTS
            }
        );
    }

    #[test]
    fn the_commitment_hash_binds_sender_plaintext_and_nonce() {
        let sender = H160::repeat_byte(3);
        let nonce = [9u8; 32];
        let base = commitment_hash(sender, b"payload", &nonce);
        assert_eq!(
            base,
            commitment_hash(sender, b"payload", &nonce),
            "the hash is a function"
        );
        assert_ne!(
            base,
            commitment_hash(H160::repeat_byte(4), b"payload", &nonce),
            "the sender is bound"
        );
        assert_ne!(
            base,
            commitment_hash(sender, b"payloae", &nonce),
            "the plaintext is bound"
        );
        assert_ne!(
            base,
            commitment_hash(sender, b"payload", &[8u8; 32]),
            "the nonce is bound"
        );
    }
}
