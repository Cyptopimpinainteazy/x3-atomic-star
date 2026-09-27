//! Clock-warped expiry lifecycle for the `x3_htlc` SBF program.
//!
//! The live validator gate exercises this program for real, but a validator's
//! clock advances with wall time and `--warp-slot` moves only the slot counter,
//! so the post-expiry refund cannot be reached there: the program requires a
//! timelock at least one hour out and refuses an earlier refund. This suite
//! closes that branch by running the same compiled artifact inside
//! `solana-program-test` and overriding the clock sysvar.
//!
//! Nothing here is a mock: the SBF artifact is the one `cargo build-sbf`
//! produces, the SPL Token program is the real one shipped with
//! `solana-program-test`, tokens really move between real token accounts, and
//! every rejection is asserted as the exact Anchor error code the runtime
//! returned.
//!
//! Branches covered here:
//!   * refund before the timelock is refused (`TimelockNotExpired` = 6006)
//!   * refund after the timelock succeeds, drains the vault, credits exactly
//!     `amount` back to the initiator and leaves the preimage zeroed
//!   * a second refund is refused (`HtlcNotRefundable` = 6005)
//!   * a claim after the refund is refused (`HtlcNotClaimable` = 6004)
//!   * a claim after the timelock but before any refund still succeeds, because
//!     the program gates a claim on status rather than on the clock; asserted
//!     explicitly so that rule cannot change unnoticed

use solana_program_test::ProgramTest;
use solana_sdk::account::Account;
use solana_sdk::clock::Clock;
use solana_sdk::instruction::{AccountMeta, Instruction};
use solana_sdk::pubkey::Pubkey;
use solana_sdk::signature::{Keypair, Signer};
use solana_sdk::transaction::Transaction;
use x3_htlc_client::{htlc_addresses, X3_HTLC_PROGRAM_ID};

/// Classic SPL Token program (Tokenkeg...), the same one the live gate uses.
const TOKEN_PROGRAM_ID: Pubkey =
    Pubkey::from_str_const("TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA");

const MINT_LEN: usize = 82;
const TOKEN_ACCOUNT_LEN: usize = 165;

/// Anchor numbers program errors from 6000 (`ERROR_CODE_OFFSET`) in declaration
/// order, so `HtlcError`'s variants map onto these.
const ERR_TIMELOCK_NOT_EXPIRED: u32 = 6006;
const ERR_HTLC_NOT_CLAIMABLE: u32 = 6004;
const ERR_HTLC_NOT_REFUNDABLE: u32 = 6005;

const PROGRAM_SO: &str = "x3_htlc";

// SPL Token instruction tags (`spl_token::instruction::TokenInstruction`).
const IX_MINT_TO: u8 = 7;
const IX_INITIALIZE_ACCOUNT3: u8 = 18;
const IX_INITIALIZE_MINT2: u8 = 20;

/// `InitializeMint2 { decimals, mint_authority, freeze_authority: None }`.
fn initialize_mint2(mint: &Pubkey, decimals: u8, authority: &Pubkey) -> Instruction {
    let mut data = vec![IX_INITIALIZE_MINT2, decimals];
    data.extend_from_slice(authority.as_ref());
    data.push(0); // COption::None for the freeze authority
    Instruction::new_with_bytes(
        TOKEN_PROGRAM_ID,
        &data,
        vec![AccountMeta::new(*mint, false)],
    )
}

/// `InitializeAccount3 { owner }`.
fn initialize_account3(account: &Pubkey, mint: &Pubkey, owner: &Pubkey) -> Instruction {
    let mut data = vec![IX_INITIALIZE_ACCOUNT3];
    data.extend_from_slice(owner.as_ref());
    Instruction::new_with_bytes(
        TOKEN_PROGRAM_ID,
        &data,
        vec![
            AccountMeta::new(*account, false),
            AccountMeta::new_readonly(*mint, false),
        ],
    )
}

/// `MintTo { amount }`.
fn mint_to(mint: &Pubkey, destination: &Pubkey, authority: &Pubkey, amount: u64) -> Instruction {
    let mut data = vec![IX_MINT_TO];
    data.extend_from_slice(&amount.to_le_bytes());
    Instruction::new_with_bytes(
        TOKEN_PROGRAM_ID,
        &data,
        vec![
            AccountMeta::new(*mint, false),
            AccountMeta::new(*destination, false),
            AccountMeta::new_readonly(*authority, true),
        ],
    )
}

/// Minimal byte-offset view of the on-chain `Htlc` account (Borsh layout, the
/// same offsets the live gate decodes from raw account bytes).
struct HtlcView<'a>(&'a [u8]);

impl<'a> HtlcView<'a> {
    fn new(data: &'a [u8]) -> Self {
        Self(data)
    }
    fn amount(&self) -> u64 {
        u64::from_le_bytes(self.0[104..112].try_into().expect("amount slice"))
    }
    fn timelock(&self) -> i64 {
        i64::from_le_bytes(self.0[144..152].try_into().expect("timelock slice"))
    }
    fn status(&self) -> u8 {
        self.0[152]
    }
    fn preimage(&self) -> [u8; 32] {
        self.0[153..185].try_into().expect("preimage slice")
    }
}

fn token_amount(account: &Account) -> u64 {
    u64::from_le_bytes(account.data[64..72].try_into().expect("amount slice"))
}

fn token_account(lamports: u64) -> Account {
    Account {
        lamports,
        data: vec![0u8; TOKEN_ACCOUNT_LEN],
        owner: TOKEN_PROGRAM_ID,
        executable: false,
        rent_epoch: 0,
    }
}

async fn send(
    ctx: &mut solana_program_test::ProgramTestContext,
    instructions: &[Instruction],
    extra_signers: &[&Keypair],
) -> Result<(), solana_program_test::BanksClientError> {
    let blockhash = ctx.get_new_latest_blockhash().await.expect("blockhash");
    let payer = ctx.payer.insecure_clone();
    let mut signers: Vec<&Keypair> = vec![&payer];
    signers.extend_from_slice(extra_signers);
    let tx = Transaction::new_signed_with_payer(
        instructions,
        Some(&payer.pubkey()),
        &signers,
        blockhash,
    );
    ctx.banks_client.process_transaction(tx).await
}

fn expect_custom_error(
    result: Result<(), solana_program_test::BanksClientError>,
    expected: u32,
    what: &str,
) {
    match result {
        Err(solana_program_test::BanksClientError::TransactionError(
            solana_sdk::transaction::TransactionError::InstructionError(
                0,
                solana_sdk::instruction::InstructionError::Custom(code),
            ),
        )) => assert_eq!(code, expected, "{what}: unexpected program error code"),
        other => panic!("{what}: expected custom program error {expected}, got {other:?}"),
    }
}

async fn htlc_account(
    ctx: &solana_program_test::ProgramTestContext,
    address: &Pubkey,
) -> Account {
    ctx.banks_client
        .get_account(*address)
        .await
        .expect("get_account")
        .unwrap_or_else(|| panic!("escrow {address} does not exist"))
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn expiry_lifecycle_on_the_compiled_program() {
    let mut program_test = ProgramTest::new(PROGRAM_SO, X3_HTLC_PROGRAM_ID, None);
    // Execute the real SBF artifact found through `BPF_OUT_DIR`.
    program_test.prefer_bpf(true);

    // The real SPL Token / ATA / Memo programs, exactly as a cluster has them.
    let rent = solana_sdk::rent::Rent::default();
    for (program_id, account) in solana_program_test::programs::spl_programs(&rent) {
        program_test.add_account(program_id, Account::from(account));
    }

    let mint = Pubkey::new_unique();
    let initiator_token = Pubkey::new_unique();
    let recipient_token = Pubkey::new_unique();
    let funded = rent.minimum_balance(TOKEN_ACCOUNT_LEN).max(1_000_000);
    program_test.add_account(
        mint,
        Account {
            lamports: rent.minimum_balance(MINT_LEN).max(1_000_000),
            data: vec![0u8; MINT_LEN],
            owner: TOKEN_PROGRAM_ID,
            executable: false,
            rent_epoch: 0,
        },
    );
    program_test.add_account(initiator_token, token_account(funded));
    program_test.add_account(recipient_token, token_account(funded));

    let mut ctx = program_test.start_with_context().await;
    let initiator = ctx.payer.insecure_clone();
    let recipient = Keypair::new();

    const DECIMALS: u8 = 0;
    const MINTED: u64 = 1_000;
    const LOCKED: u64 = 500;

    send(
        &mut ctx,
        &[
            initialize_mint2(&mint, DECIMALS, &initiator.pubkey()),
            initialize_account3(&initiator_token, &mint, &initiator.pubkey()),
            initialize_account3(&recipient_token, &mint, &recipient.pubkey()),
        ],
        &[],
    )
    .await
    .expect("SPL setup (mint + two token accounts)");
    send(
        &mut ctx,
        &[mint_to(
            &mint,
            &initiator_token,
            &initiator.pubkey(),
            MINTED,
        )],
        &[],
    )
    .await
    .expect("mint_to");

    let clock: Clock = ctx.banks_client.get_sysvar().await.expect("clock sysvar");
    let now = clock.unix_timestamp;

    // ── escrow A: the refund path ─────────────────────────────────────────
    let preimage_a = [0x11u8; 32];
    let hashlock_a = solana_sdk::hash::hash(&preimage_a).to_bytes();
    let timelock_a = now + 3_601; // just past the program's 1-hour minimum
    let a = htlc_addresses(
        &X3_HTLC_PROGRAM_ID,
        &initiator.pubkey(),
        &recipient.pubkey(),
        &hashlock_a,
    );

    send(
        &mut ctx,
        &[x3_htlc_client::build_create_htlc_ix(
            &X3_HTLC_PROGRAM_ID,
            &initiator.pubkey(),
            &recipient.pubkey(),
            &mint,
            &initiator_token,
            &hashlock_a,
            timelock_a,
            LOCKED,
        )],
        &[],
    )
    .await
    .expect("create_htlc");

    let escrow = htlc_account(&ctx, &a.htlc).await;
    let view = HtlcView::new(&escrow.data);
    assert_eq!(view.status(), 1, "escrow A must be Funded after create");
    assert_eq!(view.amount(), LOCKED);
    assert_eq!(view.timelock(), timelock_a);
    assert_eq!(
        view.preimage(),
        [0u8; 32],
        "preimage is zeroed until claimed"
    );
    assert_eq!(
        token_amount(&htlc_account(&ctx, &a.htlc_vault).await),
        LOCKED,
        "vault must hold the locked amount"
    );
    assert_eq!(
        token_amount(&htlc_account(&ctx, &initiator_token).await),
        MINTED - LOCKED,
        "initiator must have paid exactly the locked amount"
    );

    // Before the timelock expires the initiator cannot refund.
    expect_custom_error(
        send(
            &mut ctx,
            &[x3_htlc_client::build_refund_htlc_ix(
                &X3_HTLC_PROGRAM_ID,
                &initiator.pubkey(),
                &a.htlc,
                &initiator_token,
            )],
            &[],
        )
        .await,
        ERR_TIMELOCK_NOT_EXPIRED,
        "refund before the timelock",
    );
    assert_eq!(
        HtlcView::new(&htlc_account(&ctx, &a.htlc).await.data).status(),
        1
    );

    // Warp the chain clock past the timelock: the state a live validator can
    // only reach after an hour of wall time.
    let mut warped: Clock = ctx.banks_client.get_sysvar().await.expect("clock sysvar");
    warped.unix_timestamp = timelock_a + 1;
    warped.slot += 100;
    ctx.set_sysvar(&warped);
    let readback: Clock = ctx.banks_client.get_sysvar().await.expect("clock sysvar");
    assert_eq!(
        readback.unix_timestamp,
        timelock_a + 1,
        "clock override must be visible to the runtime"
    );

    send(
        &mut ctx,
        &[x3_htlc_client::build_refund_htlc_ix(
            &X3_HTLC_PROGRAM_ID,
            &initiator.pubkey(),
            &a.htlc,
            &initiator_token,
        )],
        &[],
    )
    .await
    .expect("refund after the timelock expires");

    let refunded = htlc_account(&ctx, &a.htlc).await;
    let view = HtlcView::new(&refunded.data);
    assert_eq!(view.status(), 3, "escrow A must be Refunded");
    assert_eq!(
        view.preimage(),
        [0u8; 32],
        "a refund never reveals a preimage"
    );
    assert_eq!(
        token_amount(&htlc_account(&ctx, &a.htlc_vault).await),
        0,
        "the refund must drain the vault"
    );
    assert_eq!(
        token_amount(&htlc_account(&ctx, &initiator_token).await),
        MINTED,
        "the refund must return exactly the locked amount to the initiator"
    );
    assert_eq!(
        token_amount(&htlc_account(&ctx, &recipient_token).await),
        0,
        "the recipient must not be paid by a refund"
    );

    expect_custom_error(
        send(
            &mut ctx,
            &[x3_htlc_client::build_refund_htlc_ix(
                &X3_HTLC_PROGRAM_ID,
                &initiator.pubkey(),
                &a.htlc,
                &initiator_token,
            )],
            &[],
        )
        .await,
        ERR_HTLC_NOT_REFUNDABLE,
        "double refund",
    );
    expect_custom_error(
        send(
            &mut ctx,
            &[x3_htlc_client::build_claim_htlc_ix(
                &X3_HTLC_PROGRAM_ID,
                &recipient.pubkey(),
                &a.htlc,
                &recipient_token,
                &preimage_a,
            )],
            &[&recipient],
        )
        .await,
        ERR_HTLC_NOT_CLAIMABLE,
        "claim after a refund",
    );
    assert_eq!(
        token_amount(&htlc_account(&ctx, &initiator_token).await)
            + token_amount(&htlc_account(&ctx, &recipient_token).await)
            + token_amount(&htlc_account(&ctx, &a.htlc_vault).await),
        MINTED,
        "supply must be conserved across the refund path"
    );

    // ── escrow B: claim after the timelock, before any refund ────────────
    let preimage_b = [0x22u8; 32];
    let hashlock_b = solana_sdk::hash::hash(&preimage_b).to_bytes();
    let clock: Clock = ctx.banks_client.get_sysvar().await.expect("clock sysvar");
    let timelock_b = clock.unix_timestamp + 3_601;
    let b = htlc_addresses(
        &X3_HTLC_PROGRAM_ID,
        &initiator.pubkey(),
        &recipient.pubkey(),
        &hashlock_b,
    );
    send(
        &mut ctx,
        &[x3_htlc_client::build_create_htlc_ix(
            &X3_HTLC_PROGRAM_ID,
            &initiator.pubkey(),
            &recipient.pubkey(),
            &mint,
            &initiator_token,
            &hashlock_b,
            timelock_b,
            LOCKED,
        )],
        &[],
    )
    .await
    .expect("create_htlc (escrow B)");

    let mut warped: Clock = ctx.banks_client.get_sysvar().await.expect("clock sysvar");
    warped.unix_timestamp = timelock_b + 1;
    warped.slot += 100;
    ctx.set_sysvar(&warped);

    // Documented behaviour: `claim_htlc` gates on status, not on the clock, so
    // the recipient can still take a funded-but-expired escrow. Asserting it
    // means a future change to that rule fails this gate loudly.
    send(
        &mut ctx,
        &[x3_htlc_client::build_claim_htlc_ix(
            &X3_HTLC_PROGRAM_ID,
            &recipient.pubkey(),
            &b.htlc,
            &recipient_token,
            &preimage_b,
        )],
        &[&recipient],
    )
    .await
    .expect("claim after the timelock, before any refund, is accepted");

    let claimed = htlc_account(&ctx, &b.htlc).await;
    let view = HtlcView::new(&claimed.data);
    assert_eq!(view.status(), 2, "escrow B must be Claimed");
    assert_eq!(view.preimage(), preimage_b, "the claim records the preimage");
    assert_eq!(
        token_amount(&htlc_account(&ctx, &recipient_token).await),
        LOCKED,
        "the claim must pay the recipient exactly the locked amount"
    );
    expect_custom_error(
        send(
            &mut ctx,
            &[x3_htlc_client::build_refund_htlc_ix(
                &X3_HTLC_PROGRAM_ID,
                &initiator.pubkey(),
                &b.htlc,
                &initiator_token,
            )],
            &[],
        )
        .await,
        ERR_HTLC_NOT_REFUNDABLE,
        "refund after a late claim",
    );
    assert_eq!(
        token_amount(&htlc_account(&ctx, &initiator_token).await)
            + token_amount(&htlc_account(&ctx, &recipient_token).await)
            + token_amount(&htlc_account(&ctx, &b.htlc_vault).await),
        MINTED,
        "supply must be conserved across the late-claim path"
    );
}
