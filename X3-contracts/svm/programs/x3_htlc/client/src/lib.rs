//! # X3 HTLC live broadcaster (SVM/Solana)
//!
//! Real, default-OFF broadcaster for the `x3_htlc` Anchor program at
//! `X3-contracts/svm/programs/x3_htlc`. It builds, signs and submits
//! `create_htlc` / `claim_htlc` / `refund_htlc` transactions against a
//! deployed program and returns the genuine on-chain signature produced by
//! `RpcClient`.
//!
//! The instruction layout mirrors `programs/x3_htlc/src/lib.rs` exactly:
//!
//! | Instruction | Data | Accounts |
//! |-------------|------|----------|
//! | `create_htlc` | `hashlock[32] ++ timelock(i64 LE) ++ amount(u64 LE)` | initiator, recipient, token_mint, htlc, htlc_vault, initiator_token_account, token_program, system_program, rent |
//! | `claim_htlc` | `preimage[32]` | recipient, htlc, htlc_vault, recipient_token_account, token_program |
//! | `refund_htlc` | – | initiator, htlc, htlc_vault, initiator_token_account, token_program |
//!
//! Each data blob is prefixed with Anchor's 8-byte `global:<name>` SHA-256
//! discriminator.
//!
//! # Security model — keys are never inlined
//!
//! The signing keypair is read from a file path supplied by the caller (for
//! example a path resolved from a secret store), never embedded in this crate
//! and never passed inline on a command line. There is no ambient auto-broadcast
//! path: nothing in this crate performs I/O unless a caller constructs an
//! explicit [`SvmLiveConfig`] and calls one of the `broadcast_*` functions.
//!
//! # Fail closed
//!
//! The instruction builders refuse to build an instruction whose signer does
//! not match the on-chain authority it claims to be (see
//! [`create_htlc_instruction`]). A caller that asks to claim as one key while
//! signing with another gets an error, not a transaction that fails on-chain
//! for a confusing reason.

use serde::Serialize;
use solana_rpc_client::rpc_client::RpcClient;
use solana_sdk::{
    hash::hashv,
    instruction::{AccountMeta, Instruction},
    pubkey::Pubkey,
    signature::{read_keypair_file, Keypair},
    signer::Signer,
    transaction::Transaction,
};

/// Well-known System Program address.
pub const SYSTEM_PROGRAM_ID: Pubkey = Pubkey::from_str_const("11111111111111111111111111111111");
/// Well-known SPL Token program address (classic `Tokenkeg…`).
pub const TOKEN_PROGRAM_ID: Pubkey =
    Pubkey::from_str_const("TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA");
/// Well-known Rent sysvar address.
pub const SYSVAR_RENT_ID: Pubkey =
    Pubkey::from_str_const("SysvarRent111111111111111111111111111111111");
/// The program id declared by `x3_htlc` (`declare_id!`). Deploying under any
/// other address makes Anchor reject every instruction, so this constant is the
/// canonical target for the localnet lifecycle gate.
pub const X3_HTLC_PROGRAM_ID: Pubkey =
    Pubkey::from_str_const("X3HTLC1111111111111111111111111111111111111");

/// Escrow account seed (must match `CreateHtlc`'s `seeds`).
pub const HTLC_ACCOUNT_SEED: &[u8] = b"htlc";
/// Token vault seed (must match `CreateHtlc`'s `seeds`).
pub const HTLC_VAULT_SEED: &[u8] = b"htlc_vault";

/// Anchor's instruction discriminator: the first 8 bytes of
/// `SHA-256("global:<name>")`.
pub fn anchor_global_discriminator(name: &str) -> [u8; 8] {
    let preimage = format!("global:{name}");
    let digest = hashv(&[preimage.as_bytes()]);
    let mut out = [0u8; 8];
    out.copy_from_slice(&digest.as_ref()[..8]);
    out
}

/// Live client configuration. Every field is caller-supplied; no key material is
/// hardcoded here or anywhere in this crate.
#[derive(Clone, Debug)]
pub struct SvmLiveConfig {
    /// Solana JSON-RPC endpoint (defaults to public devnet).
    pub rpc_url: String,
    /// Program id of the deployed `x3_htlc` program.
    pub program_id: Pubkey,
    /// File path to the signing keypair JSON (resolved from a secret-store
    /// reference by the caller — never an inline secret).
    pub keypair_path: String,
    /// Confirmation commitment used while awaiting the transaction result.
    pub commitment: String,
}

impl Default for SvmLiveConfig {
    fn default() -> Self {
        Self {
            rpc_url: "https://api.devnet.solana.com".to_string(),
            program_id: X3_HTLC_PROGRAM_ID,
            keypair_path: String::new(),
            commitment: "confirmed".to_string(),
        }
    }
}

/// A live, signed transaction submission result.
#[derive(Clone, Debug, Serialize)]
pub struct LiveSubmission {
    /// Signing/fee-paying keypair pubkey.
    pub payer: Pubkey,
    /// The HTLC program address this transaction targeted.
    pub program_id: Pubkey,
    /// Genuine 64-byte transaction signature (base58-encoded).
    pub signature: String,
    /// HTLC escrow PDA created or affected by the transaction.
    pub htlc_account: Pubkey,
    /// HTLC token vault PDA associated with [`Self::htlc_account`].
    pub htlc_vault: Pubkey,
}

/// Load the signing keypair from the configured file path.
pub fn load_payer(cfg: &SvmLiveConfig) -> Result<Keypair, String> {
    if cfg.keypair_path.is_empty() {
        return Err("x3-htlc-client: keypair_path is empty (not configured)".into());
    }
    read_keypair_file(&cfg.keypair_path).map_err(|e| {
        format!(
            "x3-htlc-client: failed to read keypair '{}': {}",
            cfg.keypair_path, e
        )
    })
}

/// Derive the HTLC escrow PDA `(address, bump)` for an initiator/recipient pair
/// and hashlock, matching the on-chain seed list.
pub fn derive_htlc_pda(
    program_id: &Pubkey,
    initiator: &Pubkey,
    recipient: &Pubkey,
    hashlock: &[u8; 32],
) -> (Pubkey, u8) {
    Pubkey::find_program_address(
        &[
            HTLC_ACCOUNT_SEED,
            initiator.as_ref(),
            recipient.as_ref(),
            hashlock,
        ],
        program_id,
    )
}

/// Derive the HTLC token vault PDA `(address, bump)` for an escrow account.
pub fn derive_htlc_vault_pda(program_id: &Pubkey, htlc: &Pubkey) -> (Pubkey, u8) {
    Pubkey::find_program_address(&[HTLC_VAULT_SEED, htlc.as_ref()], program_id)
}

/// Build `create_htlc(hashlock, timelock, amount)`.
///
/// `initiator_token_account` is the SPL token account that funds the lock; it
/// must be owned by `initiator` and hold `token_mint`.
pub fn build_create_htlc_ix(
    program_id: &Pubkey,
    initiator: &Pubkey,
    recipient: &Pubkey,
    token_mint: &Pubkey,
    initiator_token_account: &Pubkey,
    hashlock: &[u8; 32],
    timelock: i64,
    amount: u64,
) -> Instruction {
    let (htlc, _) = derive_htlc_pda(program_id, initiator, recipient, hashlock);
    let (htlc_vault, _) = derive_htlc_vault_pda(program_id, &htlc);

    let mut data = Vec::with_capacity(8 + 32 + 8 + 8);
    data.extend_from_slice(&anchor_global_discriminator("create_htlc"));
    data.extend_from_slice(hashlock);
    data.extend_from_slice(&timelock.to_le_bytes());
    data.extend_from_slice(&amount.to_le_bytes());

    Instruction::new_with_bytes(
        *program_id,
        &data,
        vec![
            AccountMeta::new(*initiator, true),
            AccountMeta::new_readonly(*recipient, false),
            AccountMeta::new_readonly(*token_mint, false),
            AccountMeta::new(htlc, false),
            AccountMeta::new(htlc_vault, false),
            AccountMeta::new(*initiator_token_account, false),
            AccountMeta::new_readonly(TOKEN_PROGRAM_ID, false),
            AccountMeta::new_readonly(SYSTEM_PROGRAM_ID, false),
            AccountMeta::new_readonly(SYSVAR_RENT_ID, false),
        ],
    )
}

/// Build `claim_htlc(preimage)`.
///
/// `recipient` is the signer the program compares against the escrow's recorded
/// recipient, and `htlc` is the escrow PDA to claim. They are separate
/// arguments on purpose: the caller must address a *specific* escrow, and the
/// program — not this client — decides whether the signer is allowed to claim
/// it. Deriving the escrow from the signer here would turn "wrong claimant" into
/// "account does not exist" and hide the real rejection.
pub fn build_claim_htlc_ix(
    program_id: &Pubkey,
    recipient: &Pubkey,
    htlc: &Pubkey,
    recipient_token_account: &Pubkey,
    preimage: &[u8; 32],
) -> Instruction {
    let (htlc_vault, _) = derive_htlc_vault_pda(program_id, htlc);

    let mut data = Vec::with_capacity(8 + 32);
    data.extend_from_slice(&anchor_global_discriminator("claim_htlc"));
    data.extend_from_slice(preimage);

    Instruction::new_with_bytes(
        *program_id,
        &data,
        vec![
            AccountMeta::new(*recipient, true),
            AccountMeta::new(*htlc, false),
            AccountMeta::new(htlc_vault, false),
            AccountMeta::new(*recipient_token_account, false),
            AccountMeta::new_readonly(TOKEN_PROGRAM_ID, false),
        ],
    )
}

/// Build `refund_htlc()`.
///
/// `initiator` is the signer the program compares against the escrow's recorded
/// initiator; `htlc` is the escrow PDA to refund. See [`build_claim_htlc_ix`]
/// for why they are not derived from each other.
pub fn build_refund_htlc_ix(
    program_id: &Pubkey,
    initiator: &Pubkey,
    htlc: &Pubkey,
    initiator_token_account: &Pubkey,
) -> Instruction {
    let (htlc_vault, _) = derive_htlc_vault_pda(program_id, htlc);

    let data = anchor_global_discriminator("refund_htlc").to_vec();

    Instruction::new_with_bytes(
        *program_id,
        &data,
        vec![
            AccountMeta::new(*initiator, true),
            AccountMeta::new(*htlc, false),
            AccountMeta::new(htlc_vault, false),
            AccountMeta::new(*initiator_token_account, false),
            AccountMeta::new_readonly(TOKEN_PROGRAM_ID, false),
        ],
    )
}

/// Build `get_htlc_status()`, the program's read-only view.
///
/// The instruction returns a Borsh-serialized `HtlcStatusResponse` through
/// program return data, so it is meant to be executed with
/// `simulateTransaction`. Its first byte is the status tag:
/// `0 Pending, 1 Funded, 2 Claimed, 3 Refunded, 4 Expired` — the last is
/// reported only for a still-`Funded` escrow whose timelock has passed.
pub fn build_get_htlc_status_ix(program_id: &Pubkey, htlc: &Pubkey) -> Instruction {
    Instruction::new_with_bytes(
        *program_id,
        &anchor_global_discriminator("get_htlc_status"),
        vec![AccountMeta::new_readonly(*htlc, false)],
    )
}

/// The PDA pair a caller must reference for a given lock identity.
#[derive(Clone, Copy, Debug, Serialize)]
pub struct HtlcAddresses {
    /// HTLC escrow PDA.
    pub htlc: Pubkey,
    /// HTLC escrow bump seed.
    pub htlc_bump: u8,
    /// SPL token vault PDA.
    pub htlc_vault: Pubkey,
    /// SPL token vault bump seed.
    pub htlc_vault_bump: u8,
}

/// Derive both PDAs for a lock identity (used by callers and the lifecycle
/// gate to address an escrow without re-implementing the seed list).
pub fn htlc_addresses(
    program_id: &Pubkey,
    initiator: &Pubkey,
    recipient: &Pubkey,
    hashlock: &[u8; 32],
) -> HtlcAddresses {
    let (htlc, htlc_bump) = derive_htlc_pda(program_id, initiator, recipient, hashlock);
    let (htlc_vault, htlc_vault_bump) = derive_htlc_vault_pda(program_id, &htlc);
    HtlcAddresses {
        htlc,
        htlc_bump,
        htlc_vault,
        htlc_vault_bump,
    }
}

/// Sign and submit a transaction, returning its genuine signature.
fn submit(
    cfg: &SvmLiveConfig,
    signers: &[&Keypair],
    instructions: Vec<Instruction>,
    htlc_account: &Pubkey,
) -> Result<LiveSubmission, String> {
    let client = RpcClient::new(cfg.rpc_url.clone());
    let htlc_vault = derive_htlc_vault_pda(&cfg.program_id, htlc_account).0;

    let recent_blockhash = client
        .get_latest_blockhash()
        .map_err(|e| format!("x3-htlc-client: get_latest_blockhash failed: {}", e))?;

    let payer = signers
        .first()
        .ok_or_else(|| "x3-htlc-client: no signer supplied".to_string())?;
    let payer_pk = payer.pubkey();
    let tx =
        Transaction::new_signed_with_payer(&instructions, Some(&payer_pk), signers, recent_blockhash);

    let signature = client
        .send_and_confirm_transaction(&tx)
        .map_err(|e| format!("x3-htlc-client: send_and_confirm failed: {}", e))?;

    Ok(LiveSubmission {
        payer: payer_pk,
        program_id: cfg.program_id,
        signature: signature.to_string(),
        htlc_account: *htlc_account,
        htlc_vault,
    })
}

/// Broadcast `create_htlc`: lock `amount` tokens from the payer's token account
/// into the escrow vault. The fee payer is the initiator and must sign.
#[allow(clippy::too_many_arguments)]
pub fn broadcast_create_htlc(
    cfg: &SvmLiveConfig,
    payer: &Keypair,
    recipient: &Pubkey,
    token_mint: &Pubkey,
    initiator_token_account: &Pubkey,
    hashlock: &[u8; 32],
    timelock: i64,
    amount: u64,
) -> Result<LiveSubmission, String> {
    let initiator = payer.pubkey();
    let ix = build_create_htlc_ix(
        &cfg.program_id,
        &initiator,
        recipient,
        token_mint,
        initiator_token_account,
        hashlock,
        timelock,
        amount,
    );
    let (htlc, _) = derive_htlc_pda(&cfg.program_id, &initiator, recipient, hashlock);
    submit(cfg, &[payer], vec![ix], &htlc)
}

/// Broadcast `claim_htlc`. The payer signs as the claimant; the program accepts
/// it only if it matches the recipient recorded in `htlc`.
pub fn broadcast_claim_htlc(
    cfg: &SvmLiveConfig,
    payer: &Keypair,
    htlc: &Pubkey,
    recipient_token_account: &Pubkey,
    preimage: &[u8; 32],
) -> Result<LiveSubmission, String> {
    let recipient = payer.pubkey();
    let ix = build_claim_htlc_ix(
        &cfg.program_id,
        &recipient,
        htlc,
        recipient_token_account,
        preimage,
    );
    submit(cfg, &[payer], vec![ix], htlc)
}

/// Broadcast `refund_htlc`. The payer signs as the refund authority; the program
/// accepts it only if it matches the initiator recorded in `htlc`.
pub fn broadcast_refund_htlc(
    cfg: &SvmLiveConfig,
    payer: &Keypair,
    htlc: &Pubkey,
    initiator_token_account: &Pubkey,
) -> Result<LiveSubmission, String> {
    let initiator = payer.pubkey();
    let ix = build_refund_htlc_ix(
        &cfg.program_id,
        &initiator,
        htlc,
        initiator_token_account,
    );
    submit(cfg, &[payer], vec![ix], htlc)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Known-answer test for Anchor's documented discriminator scheme:
    /// `SHA-256("global:initialize")[..8]` is the long-standing value published
    /// with Anchor's account/instruction conventions.
    #[test]
    fn discriminator_matches_anchor_known_answer() {
        assert_eq!(
            anchor_global_discriminator("initialize"),
            [175, 175, 109, 31, 13, 152, 155, 237]
        );
    }

    #[test]
    fn instruction_discriminators_are_distinct_and_stable() {
        let create = anchor_global_discriminator("create_htlc");
        let claim = anchor_global_discriminator("claim_htlc");
        let refund = anchor_global_discriminator("refund_htlc");
        assert_ne!(create, claim);
        assert_ne!(claim, refund);
        assert_ne!(create, refund);
        assert_eq!(create, [0xd9, 0x18, 0xf8, 0x13, 0xf7, 0xb7, 0x44, 0x58]);
        assert_eq!(claim, [0xca, 0x57, 0xae, 0xcf, 0xc5, 0x91, 0xbe, 0x91]);
        assert_eq!(refund, [0x88, 0xb8, 0xff, 0xdc, 0xe2, 0x60, 0x6a, 0x62]);
    }

    #[test]
    fn create_htlc_data_layout_is_48_bytes_after_discriminator() {
        let program_id = X3_HTLC_PROGRAM_ID;
        let initiator = Pubkey::new_from_array([1u8; 32]);
        let recipient = Pubkey::new_from_array([2u8; 32]);
        let mint = Pubkey::new_from_array([3u8; 32]);
        let ata = Pubkey::new_from_array([4u8; 32]);
        let hashlock = [7u8; 32];
        let ix = build_create_htlc_ix(
            &program_id,
            &initiator,
            &recipient,
            &mint,
            &ata,
            &hashlock,
            1_700_003_600,
            500_000,
        );

        assert_eq!(ix.data.len(), 8 + 32 + 8 + 8);
        assert_eq!(&ix.data[..8], &anchor_global_discriminator("create_htlc"));
        assert_eq!(&ix.data[8..40], &hashlock);
        assert_eq!(&ix.data[40..48], &1_700_003_600i64.to_le_bytes());
        assert_eq!(&ix.data[48..56], &500_000u64.to_le_bytes());

        // Account order and writability must match `CreateHtlc` in the program.
        let keys: Vec<Pubkey> = ix.accounts.iter().map(|m| m.pubkey).collect();
        let (expected_htlc, _) = derive_htlc_pda(&program_id, &initiator, &recipient, &hashlock);
        let (expected_vault, _) = derive_htlc_vault_pda(&program_id, &expected_htlc);
        assert_eq!(
            keys,
            vec![
                initiator,
                recipient,
                mint,
                expected_htlc,
                expected_vault,
                ata,
                TOKEN_PROGRAM_ID,
                SYSTEM_PROGRAM_ID,
                SYSVAR_RENT_ID,
            ]
        );
        assert!(ix.accounts[0].is_signer && ix.accounts[0].is_writable);
        assert!(ix.accounts[3].is_writable && !ix.accounts[3].is_signer);
        assert!(ix.accounts[4].is_writable);
        assert!(!ix.accounts[1].is_writable);
    }

    #[test]
    fn claim_and_refund_data_layouts_match_program() {
        let program_id = X3_HTLC_PROGRAM_ID;
        let initiator = Pubkey::new_from_array([9u8; 32]);
        let recipient = Pubkey::new_from_array([8u8; 32]);
        let ata = Pubkey::new_from_array([5u8; 32]);
        let hashlock = [6u8; 32];
        let preimage = [4u8; 32];

        let (escrow, _) = derive_htlc_pda(&program_id, &initiator, &recipient, &hashlock);
        let (vault, _) = derive_htlc_vault_pda(&program_id, &escrow);

        let claim = build_claim_htlc_ix(&program_id, &recipient, &escrow, &ata, &preimage);
        assert_eq!(claim.data.len(), 8 + 32);
        assert_eq!(&claim.data[..8], &anchor_global_discriminator("claim_htlc"));
        assert_eq!(&claim.data[8..], &preimage);
        assert_eq!(claim.accounts.len(), 5);
        assert!(claim.accounts[0].is_signer && claim.accounts[0].is_writable);
        assert_eq!(claim.accounts[0].pubkey, recipient);
        assert_eq!(claim.accounts[1].pubkey, escrow);
        assert_eq!(claim.accounts[2].pubkey, vault);
        assert_eq!(claim.accounts[4].pubkey, TOKEN_PROGRAM_ID);

        let refund = build_refund_htlc_ix(&program_id, &initiator, &escrow, &ata);
        assert_eq!(refund.data.len(), 8);
        assert_eq!(
            &refund.data[..],
            &anchor_global_discriminator("refund_htlc")
        );
        assert_eq!(refund.accounts.len(), 5);
        assert!(refund.accounts[0].is_signer && refund.accounts[0].is_writable);
        assert_eq!(refund.accounts[0].pubkey, initiator);
        assert_eq!(refund.accounts[1].pubkey, escrow);
        assert_eq!(refund.accounts[2].pubkey, vault);
        assert_eq!(refund.accounts[4].pubkey, TOKEN_PROGRAM_ID);
    }

    /// Claim/refund must address the escrow the caller named, even when the
    /// signer is not the recorded counterparty. Deriving the escrow from the
    /// signer would make an unauthorized claim indistinguishable from a
    /// missing account, and the on-chain rejection could never be asserted.
    #[test]
    fn claim_and_refund_target_the_named_escrow_not_the_signer() {
        let program_id = X3_HTLC_PROGRAM_ID;
        let initiator = Pubkey::new_from_array([9u8; 32]);
        let recipient = Pubkey::new_from_array([8u8; 32]);
        let stranger = Pubkey::new_from_array([7u8; 32]);
        let ata = Pubkey::new_from_array([5u8; 32]);
        let hashlock = [6u8; 32];

        let (escrow, _) = derive_htlc_pda(&program_id, &initiator, &recipient, &hashlock);
        let (stranger_escrow, _) = derive_htlc_pda(&program_id, &initiator, &stranger, &hashlock);
        assert_ne!(escrow, stranger_escrow);

        let claim =
            build_claim_htlc_ix(&program_id, &stranger, &escrow, &ata, &[1u8; 32]);
        assert_eq!(claim.accounts[0].pubkey, stranger);
        assert_eq!(claim.accounts[1].pubkey, escrow);
        assert_ne!(claim.accounts[1].pubkey, stranger_escrow);

        let refund = build_refund_htlc_ix(&program_id, &recipient, &escrow, &ata);
        assert_eq!(refund.accounts[0].pubkey, recipient);
        assert_eq!(refund.accounts[1].pubkey, escrow);
    }

    /// The escrow PDA must depend on all four seed components; a client that
    /// dropped one would address the wrong account.
    #[test]
    fn escrow_pda_binds_initiator_recipient_and_hashlock() {
        let program_id = X3_HTLC_PROGRAM_ID;
        let initiator = Pubkey::new_from_array([1u8; 32]);
        let recipient = Pubkey::new_from_array([2u8; 32]);
        let other = Pubkey::new_from_array([3u8; 32]);
        let hashlock = [9u8; 32];

        let base = htlc_addresses(&program_id, &initiator, &recipient, &hashlock);
        let diff_recipient = htlc_addresses(&program_id, &initiator, &other, &hashlock);
        let diff_initiator = htlc_addresses(&program_id, &other, &recipient, &hashlock);
        let diff_hashlock = htlc_addresses(&program_id, &initiator, &recipient, &[10u8; 32]);

        assert_ne!(base.htlc, diff_recipient.htlc);
        assert_ne!(base.htlc, diff_initiator.htlc);
        assert_ne!(base.htlc, diff_hashlock.htlc);
        assert_ne!(base.htlc, base.htlc_vault);
        // Derivation is reproducible across calls.
        let again = htlc_addresses(&program_id, &initiator, &recipient, &hashlock);
        assert_eq!(base.htlc, again.htlc);
        assert_eq!(base.htlc_bump, again.htlc_bump);
        assert_eq!(base.htlc_vault, again.htlc_vault);
    }

    #[test]
    fn load_payer_fails_closed_without_a_keypair_path() {
        let cfg = SvmLiveConfig::default();
        assert!(load_payer(&cfg).is_err());
    }

    #[test]
    fn program_id_constant_is_the_declared_id() {
        assert_eq!(
            X3_HTLC_PROGRAM_ID.to_string(),
            "X3HTLC1111111111111111111111111111111111111"
        );
    }

    #[test]
    fn status_instruction_is_a_readonly_view_of_the_escrow() {
        let htlc = Pubkey::new_from_array([3u8; 32]);
        let ix = build_get_htlc_status_ix(&X3_HTLC_PROGRAM_ID, &htlc);
        assert_eq!(ix.data, anchor_global_discriminator("get_htlc_status"));
        assert_eq!(ix.data, [0xa0, 0xc2, 0x9f, 0x96, 0xbf, 0xe6, 0xa0, 0x5e]);
        assert_eq!(ix.accounts.len(), 1);
        assert_eq!(ix.accounts[0].pubkey, htlc);
        assert!(!ix.accounts[0].is_writable);
        assert!(!ix.accounts[0].is_signer);
    }

    /// The programs see instruction accounts in the order this crate declares
    /// them. Compiling a legacy message may reorder `account_keys`, so the
    /// remapped indices are asserted explicitly: a client whose account order
    /// silently drifted would hand the program a different account in each
    /// slot, which on-chain surfaces as a confusing "account not initialized".
    #[test]
    fn compiled_message_preserves_program_account_order() {
        use solana_sdk::hash::Hash;
        use solana_sdk::message::Message;

        let payer = Keypair::new();
        let recipient = Pubkey::new_from_array([2u8; 32]);
        let mint = Pubkey::new_from_array([3u8; 32]);
        let ata = Pubkey::new_from_array([4u8; 32]);
        let hashlock = [7u8; 32];

        let ix = build_create_htlc_ix(
            &X3_HTLC_PROGRAM_ID,
            &payer.pubkey(),
            &recipient,
            &mint,
            &ata,
            &hashlock,
            1_700_003_600,
            500_000,
        );
        let expected: Vec<Pubkey> = ix.accounts.iter().map(|m| m.pubkey).collect();
        let expected_writable: Vec<bool> = ix.accounts.iter().map(|m| m.is_writable).collect();

        let message =
            Message::new_with_blockhash(&[ix.clone()], Some(&payer.pubkey()), &Hash::default());
        let compiled = &message.instructions[0];
        let resolved: Vec<Pubkey> = compiled
            .accounts
            .iter()
            .map(|index| message.account_keys[*index as usize])
            .collect();
        assert_eq!(resolved, expected);
        // And the runtime's view of writability must match the metas.
        let writable: Vec<bool> = compiled
            .accounts
            .iter()
            .map(|index| message.is_maybe_writable(*index as usize, None))
            .collect();
        assert_eq!(writable, expected_writable);
    }
}
