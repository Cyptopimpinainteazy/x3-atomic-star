//! # x3-htlc-broadcast — explicit live broadcaster for the `x3_htlc` program
//!
//! Thin, explicit CLI over the `x3_htlc_client` library. It never
//! auto-broadcasts: every piece of live configuration (RPC url, program id,
//! fee-payer keypair path) and every action argument must be supplied by the
//! caller. No secret is accepted inline — the keypair is read from a file path.
//!
//! ```bash
//! x3-htlc-broadcast --rpc http://127.0.0.1:8899 --payer-keypair payer.json \
//!   create --recipient <PK> --mint <MINT> --initiator-token-account <ATA> \
//!          --hashlock <32-byte-hex> --timelock <unix-ts> --amount <base-units>
//! ```
//!
//! `claim` and `refund` must be signed by the recorded recipient / initiator
//! respectively; pass a different `--payer-keypair` to prove a rejected
//! authority. `addresses` performs pure PDA derivation and touches no network.

use std::process::ExitCode;

use solana_sdk::pubkey::Pubkey;
use solana_sdk::instruction::Instruction;
use solana_sdk::signature::{Keypair, Signer};
use solana_sdk::transaction::Transaction;
use x3_htlc_client::{
    build_claim_htlc_ix, build_create_htlc_ix, build_refund_htlc_ix, htlc_addresses, SvmLiveConfig,
    X3_HTLC_PROGRAM_ID,
};

fn pick(key: &str, a: &[String]) -> Result<String, String> {
    a.iter()
        .position(|x| x == key)
        .and_then(|i| a.get(i + 1))
        .cloned()
        .ok_or_else(|| format!("missing {key}"))
}

fn parse_pubkey(name: &str, a: &[String]) -> Result<Pubkey, String> {
    pick(name, a)?
        .parse::<Pubkey>()
        .map_err(|e| format!("{name}: bad pubkey ({e})"))
}

fn parse_hex32(name: &str, a: &[String]) -> Result<[u8; 32], String> {
    let raw = pick(name, a)?;
    let raw = raw.strip_prefix("0x").unwrap_or(&raw);
    let bytes = hex::decode(raw).map_err(|e| format!("{name}: bad hex ({e})"))?;
    if bytes.len() != 32 {
        return Err(format!("{name}: expected 32 bytes, got {}", bytes.len()));
    }
    let mut out = [0u8; 32];
    out.copy_from_slice(&bytes);
    Ok(out)
}

fn parse_u64(name: &str, a: &[String]) -> Result<u64, String> {
    pick(name, a)?
        .parse::<u64>()
        .map_err(|e| format!("{name}: {e}"))
}

fn parse_i64(name: &str, a: &[String]) -> Result<i64, String> {
    pick(name, a)?
        .parse::<i64>()
        .map_err(|e| format!("{name}: {e}"))
}

fn print_error(action: &str, error: &str, json: bool) {
    if json {
        println!(
            "{}",
            serde_json::json!({"action": action, "status": "error", "error": error})
        );
    } else {
        eprintln!("{action} failed: {error}");
    }
}

fn print_submission(action: &str, s: &x3_htlc_client::LiveSubmission, json: bool) {
    if json {
        let mut value = serde_json::to_value(s).expect("LiveSubmission is serializable");
        value["action"] = serde_json::Value::String(action.to_string());
        value["status"] = serde_json::Value::String("submitted".to_string());
        println!(
            "{}",
            serde_json::to_string(&value).expect("JSON serialization")
        );
    } else {
        println!(
            "{}_SUBMITTED sig={} htlc={} vault={} payer={}",
            action.to_ascii_uppercase(),
            s.signature,
            s.htlc_account,
            s.htlc_vault,
            s.payer
        );
    }
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let json = args.iter().any(|arg| arg == "--json");

    let Some(action_index) = args
        .iter()
        .position(|x| matches!(x.as_str(), "create" | "claim" | "refund" | "addresses"))
    else {
        eprintln!(
            "usage: x3-htlc-broadcast --rpc <url> --payer-keypair <path> [--program-id <id>] \
             [--json] <create|claim|refund|addresses> ..."
        );
        return ExitCode::from(2);
    };
    let action = args[action_index].clone();
    let rest: Vec<String> = args[action_index + 1..].to_vec();

    let program_id = match pick("--program-id", &args) {
        Ok(v) => match v.parse::<Pubkey>() {
            Ok(p) => p,
            Err(e) => {
                eprintln!("bad --program-id: {e}");
                return ExitCode::from(2);
            }
        },
        Err(_) => X3_HTLC_PROGRAM_ID,
    };

    // `addresses` is a pure, offline derivation helper.
    if action == "addresses" {
        let initiator = match parse_pubkey("--initiator", &rest) {
            Ok(v) => v,
            Err(e) => {
                print_error(&action, &e, json);
                return ExitCode::from(2);
            }
        };
        let recipient = match parse_pubkey("--recipient", &rest) {
            Ok(v) => v,
            Err(e) => {
                print_error(&action, &e, json);
                return ExitCode::from(2);
            }
        };
        let hashlock = match parse_hex32("--hashlock", &rest) {
            Ok(v) => v,
            Err(e) => {
                print_error(&action, &e, json);
                return ExitCode::from(2);
            }
        };
        let a = htlc_addresses(&program_id, &initiator, &recipient, &hashlock);
        if json {
            println!(
                "{}",
                serde_json::to_string(&a).expect("HtlcAddresses is serializable")
            );
        } else {
            println!(
                "htlc={} bump={} vault={} vault_bump={}",
                a.htlc, a.htlc_bump, a.htlc_vault, a.htlc_vault_bump
            );
        }
        return ExitCode::SUCCESS;
    }

    let rpc = match pick("--rpc", &args) {
        Ok(v) => v,
        Err(_) => {
            eprintln!("missing --rpc <url>");
            return ExitCode::from(2);
        }
    };

    // Parse every action argument before touching the network so a malformed
    // invocation is rejected without submitting anything.
    let parsed: Result<Action, String> = match action.as_str() {
        "create" => (|| {
            Ok(Action::Create {
                recipient: parse_pubkey("--recipient", &rest)?,
                mint: parse_pubkey("--mint", &rest)?,
                initiator_token_account: parse_pubkey("--initiator-token-account", &rest)?,
                hashlock: parse_hex32("--hashlock", &rest)?,
                timelock: parse_i64("--timelock", &rest)?,
                amount: parse_u64("--amount", &rest)?,
            })
        })(),
        "claim" => (|| {
            Ok(Action::Claim {
                escrow: parse_pubkey("--escrow", &rest)?,
                recipient_token_account: parse_pubkey("--recipient-token-account", &rest)?,
                preimage: parse_hex32("--preimage", &rest)?,
            })
        })(),
        "refund" => (|| {
            Ok(Action::Refund {
                escrow: parse_pubkey("--escrow", &rest)?,
                initiator_token_account: parse_pubkey("--initiator-token-account", &rest)?,
            })
        })(),
        other => Err(format!("unknown action {other}")),
    };

    let parsed = match parsed {
        Ok(p) => p,
        Err(e) => {
            print_error(&action, &e, json);
            return ExitCode::from(2);
        }
    };

    let keypair_path = match pick("--payer-keypair", &args) {
        Ok(v) => v,
        Err(_) => {
            eprintln!("missing --payer-keypair <path>");
            return ExitCode::from(2);
        }
    };
    let cfg = SvmLiveConfig {
        rpc_url: rpc,
        program_id,
        keypair_path,
        commitment: "confirmed".into(),
    };
    let payer: Keypair = match x3_htlc_client::load_payer(&cfg) {
        Ok(p) => p,
        Err(e) => {
            print_error(&action, &e, json);
            return ExitCode::from(1);
        }
    };

    // The instruction the action would submit. `--dry-run` prints it and
    // `--simulate` runs it through the runtime without broadcasting, so a
    // caller can verify account order, writability, data layout and the
    // runtime's own view of each account before spending a signature.
    let build_ix = |action: &Action| -> Instruction {
        match action {
            Action::Create {
                recipient,
                mint,
                initiator_token_account,
                hashlock,
                timelock,
                amount,
            } => build_create_htlc_ix(
                &cfg.program_id,
                &payer.pubkey(),
                recipient,
                mint,
                initiator_token_account,
                hashlock,
                *timelock,
                *amount,
            ),
            Action::Claim {
                escrow,
                recipient_token_account,
                preimage,
            } => build_claim_htlc_ix(
                &cfg.program_id,
                &payer.pubkey(),
                escrow,
                recipient_token_account,
                preimage,
            ),
            Action::Refund {
                escrow,
                initiator_token_account,
            } => build_refund_htlc_ix(
                &cfg.program_id,
                &payer.pubkey(),
                escrow,
                initiator_token_account,
            ),
        }
    };

    if args.iter().any(|a| a == "--simulate") {
        let ix = build_ix(&parsed);
        let client = solana_rpc_client::rpc_client::RpcClient::new(cfg.rpc_url.clone());
        let blockhash = match client.get_latest_blockhash() {
            Ok(b) => b,
            Err(e) => {
                print_error(&action, &format!("get_latest_blockhash: {e}"), json);
                return ExitCode::from(1);
            }
        };
        let tx = Transaction::new_signed_with_payer(
            &[ix],
            Some(&payer.pubkey()),
            &[&payer],
            blockhash,
        );
        match client.simulate_transaction(&tx) {
            Ok(sim) => {
                println!(
                    "{}",
                    serde_json::to_string_pretty(&serde_json::json!({
                        "action": action,
                        "status": "simulated",
                        "err": sim.value.err,
                        "units_consumed": sim.value.units_consumed,
                        "accounts": sim.value.accounts,
                        "logs": sim.value.logs,
                    }))
                    .expect("simulation result is serializable")
                );
                return ExitCode::SUCCESS;
            }
            Err(e) => {
                print_error(&action, &format!("simulate_transaction: {e}"), json);
                return ExitCode::from(1);
            }
        }
    } else if args.iter().any(|a| a == "--dry-run") {
        let ix = build_ix(&parsed);
        if json {
            let accounts: Vec<serde_json::Value> = ix
                .accounts
                .iter()
                .map(|m| {
                    serde_json::json!({
                        "pubkey": m.pubkey.to_string(),
                        "signer": m.is_signer,
                        "writable": m.is_writable,
                    })
                })
                .collect();
            println!(
                "{}",
                serde_json::json!({
                    "action": action,
                    "status": "dry-run",
                    "program_id": ix.program_id.to_string(),
                    "data_hex": hex::encode(&ix.data),
                    "accounts": accounts,
                })
            );
        } else {
            println!("DRY-RUN {action} program={} data={}", ix.program_id, hex::encode(&ix.data));
            for (i, m) in ix.accounts.iter().enumerate() {
                println!(
                    "  [{i}] {} signer={} writable={}",
                    m.pubkey, m.is_signer, m.is_writable
                );
            }
        }
        return ExitCode::SUCCESS;
    }

    let result = match parsed {
        Action::Create {
            recipient,
            mint,
            initiator_token_account,
            hashlock,
            timelock,
            amount,
        } => x3_htlc_client::broadcast_create_htlc(
            &cfg,
            &payer,
            &recipient,
            &mint,
            &initiator_token_account,
            &hashlock,
            timelock,
            amount,
        ),
        Action::Claim {
            escrow,
            recipient_token_account,
            preimage,
        } => x3_htlc_client::broadcast_claim_htlc(
            &cfg,
            &payer,
            &escrow,
            &recipient_token_account,
            &preimage,
        ),
        Action::Refund {
            escrow,
            initiator_token_account,
        } => x3_htlc_client::broadcast_refund_htlc(
            &cfg,
            &payer,
            &escrow,
            &initiator_token_account,
        ),
    };

    match result {
        Ok(s) => {
            print_submission(&action, &s, json);
            ExitCode::SUCCESS
        }
        Err(e) => {
            print_error(&action, &e, json);
            ExitCode::from(1)
        }
    }
}

enum Action {
    Create {
        recipient: Pubkey,
        mint: Pubkey,
        initiator_token_account: Pubkey,
        hashlock: [u8; 32],
        timelock: i64,
        amount: u64,
    },
    Claim {
        escrow: Pubkey,
        recipient_token_account: Pubkey,
        preimage: [u8; 32],
    },
    Refund {
        escrow: Pubkey,
        initiator_token_account: Pubkey,
    },
}
