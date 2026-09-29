//! Signing an artifact key registry, and loading one only if a root the reader names signed it
//! (X3-LANG-009).
//!
//! `x3 compile --sign-key-hex` and `x3 verify-artifact` decide which keys may sign an artifact from
//! a registry file. Unsigned, anyone who could edit that file could add a key of their own, and
//! every artifact that key signed would verify. `x3 sign-registry` writes a detached
//! `<registry>.sig.json` over the file's **exact bytes**, and both commands refuse a registry whose
//! signature does not verify under the root passed as `--registry-root`. An unsigned registry has
//! to be asked for by name (`--unsigned-registry`), so the default is the checked path.

use crate::commands::compile::load_artifact_registry;
use crate::error::{CliError, Result};
use clap::Args;
use colored::Colorize;
use serde::{Deserialize, Serialize};
use sp_core::Pair as _;
use std::path::{Path, PathBuf};

/// Domain separator for an artifact registry signature. Distinct from the receipt registry's
/// (`x3-receipt-key-registry-v1`), so a signature over one kind of registry is never valid as the
/// other.
pub const ARTIFACT_REGISTRY_DOMAIN: &[u8] = b"x3-artifact-key-registry-v1";

/// A detached signature over an artifact key registry file.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RegistrySignature {
    /// The signing root's ed25519 public key, 64 hex characters. A label for a reader; the trust
    /// decision is the root the verifier names.
    pub root_public_key: String,
    /// sha256 of the registry file's bytes, 64 hex characters.
    pub registry_sha256: String,
    /// ed25519 signature over [`signing_digest`], 128 hex characters.
    pub signature: String,
}

/// The digest a registry root signs: the domain, then the file's sha256.
pub fn signing_digest(registry_bytes: &[u8]) -> [u8; 32] {
    let file_digest = sp_core::hashing::sha2_256(registry_bytes);
    let mut buffer = Vec::with_capacity(ARTIFACT_REGISTRY_DOMAIN.len() + 32);
    buffer.extend_from_slice(ARTIFACT_REGISTRY_DOMAIN);
    buffer.extend_from_slice(&file_digest);
    sp_core::hashing::sha2_256(&buffer)
}

pub fn sign(registry_bytes: &[u8], root: &sp_core::ed25519::Pair) -> RegistrySignature {
    RegistrySignature {
        root_public_key: hex::encode(root.public().0),
        registry_sha256: hex::encode(sp_core::hashing::sha2_256(registry_bytes)),
        signature: hex::encode(root.sign(&signing_digest(registry_bytes)).0),
    }
}

/// Check `signature` over `registry_bytes` against `trusted_root`. Every refusal names what broke.
pub fn verify(
    registry_bytes: &[u8],
    signature: &RegistrySignature,
    trusted_root: &[u8; 32],
) -> std::result::Result<(), String> {
    if !signature
        .root_public_key
        .eq_ignore_ascii_case(&hex::encode(trusted_root))
    {
        return Err(format!(
            "the registry is signed by root {}, not the root you trust ({})",
            signature.root_public_key,
            hex::encode(trusted_root)
        ));
    }
    if !signature
        .registry_sha256
        .eq_ignore_ascii_case(&hex::encode(sp_core::hashing::sha2_256(registry_bytes)))
    {
        return Err(
            "the registry signature is about a different file: its sha256 does not match"
                .to_string(),
        );
    }
    let bytes: [u8; 64] = hex::decode(signature.signature.trim())
        .ok()
        .and_then(|bytes| bytes.try_into().ok())
        .ok_or_else(|| "registry signature: signature is not 64 bytes of hex".to_string())?;
    let ok = sp_core::ed25519::Pair::verify(
        &sp_core::ed25519::Signature::from_raw(bytes),
        signing_digest(registry_bytes),
        &sp_core::ed25519::Public::from_raw(*trusted_root),
    );
    if ok {
        Ok(())
    } else {
        Err("the registry signature does not verify under the trusted root".to_string())
    }
}

/// Where a registry's detached signature lives: `<registry>.sig.json`.
pub fn signature_path(registry: &Path) -> PathBuf {
    let mut name = registry.as_os_str().to_owned();
    name.push(".sig.json");
    PathBuf::from(name)
}

/// How a command decides to trust a registry file. Flattened into `x3 compile` and
/// `x3 verify-artifact`.
#[derive(Args, Clone, Debug, Default)]
pub struct RegistryTrust {
    /// The registry root you trust (64-hex ed25519 public key). The registry is used only if its
    /// `<registry>.sig.json` verifies under it.
    #[arg(
        long = "registry-root",
        value_name = "HEX",
        conflicts_with = "unsigned_registry"
    )]
    pub registry_root: Option<String>,

    /// Use the registry without checking a signature. Only for local development: anyone who can
    /// edit an unsigned registry decides which keys it trusts.
    #[arg(long = "unsigned-registry")]
    pub unsigned_registry: bool,
}

/// Load an artifact key registry the way `trust` says to: signed under the named root, or unsigned
/// when that was asked for by name. Neither given is a refusal, not a silent unsigned load.
pub fn load_trusted_registry(
    path: &PathBuf,
    trust: &RegistryTrust,
) -> Result<x3_common::artifact::ArtifactKeyRegistry> {
    match (&trust.registry_root, trust.unsigned_registry) {
        (Some(root_hex), _) => {
            let root: [u8; 32] = hex::decode(root_hex.trim())
                .ok()
                .and_then(|bytes| bytes.try_into().ok())
                .ok_or_else(|| {
                    CliError::Build("--registry-root must be 64 hex characters".to_string())
                })?;
            let bytes = std::fs::read(path)
                .map_err(|e| CliError::Build(format!("read {}: {e}", path.display())))?;
            let sig_path = signature_path(path);
            let body = std::fs::read_to_string(&sig_path).map_err(|e| {
                CliError::Build(format!(
                    "the registry has no signature at {}: {e}",
                    sig_path.display()
                ))
            })?;
            let signature: RegistrySignature = serde_json::from_str(&body).map_err(|e| {
                CliError::Build(format!("registry signature {}: {e}", sig_path.display()))
            })?;
            verify(&bytes, &signature, &root).map_err(CliError::Build)?;
            load_artifact_registry(path)
        }
        (None, true) => load_artifact_registry(path),
        (None, false) => Err(CliError::Build(
            "an artifact key registry must be signed: pass `--registry-root <64-hex public key>` \
             (sign it with `x3 sign-registry`), or `--unsigned-registry` for local development"
                .to_string(),
        )),
    }
}

#[derive(Args)]
pub struct SignRegistryArgs {
    /// The artifact key registry to sign.
    #[arg(required = true)]
    pub registry: PathBuf,

    /// 64-hex ed25519 seed of the registry root.
    #[arg(long)]
    pub key_hex: String,
}

pub async fn execute(args: SignRegistryArgs) -> Result<()> {
    let seed: [u8; 32] = hex::decode(args.key_hex.trim())
        .ok()
        .and_then(|bytes| bytes.try_into().ok())
        .ok_or_else(|| CliError::Build("--key-hex must be 64 hex characters".to_string()))?;
    // Refuse to sign something that is not a valid registry: a signature vouches for its contents.
    load_artifact_registry(&args.registry)?;
    let bytes = std::fs::read(&args.registry)
        .map_err(|e| CliError::Build(format!("read {}: {e}", args.registry.display())))?;
    let signature = sign(&bytes, &sp_core::ed25519::Pair::from_seed(&seed));
    let sig_path = signature_path(&args.registry);
    let json = serde_json::to_string_pretty(&signature)
        .map_err(|e| CliError::Build(format!("encode: {e}")))?;
    std::fs::write(&sig_path, json)?;
    println!(
        "{} registry signed by root {} → {}",
        "✓".green(),
        signature.root_public_key,
        sig_path.display()
    );
    Ok(())
}
