//! `x3 verify-artifact`: check a standalone `.x3b` against its detached attestation and a key
//! registry (X3-LANG-009).
//!
//! An artifact executed on chain is authenticated by the signed extrinsic that carries it. One
//! handed around out of band — copied between machines, fetched from a release page — is
//! authenticated only by this: the attestation `x3 compile --sign-key-hex` wrote next to it, checked
//! against a registry the reader trusts.

use crate::commands::compile::attestation_path;
use crate::error::{CliError, Result};
use clap::Args;
use colored::Colorize;
use std::path::PathBuf;

#[derive(Args)]
pub struct VerifyArtifactArgs {
    /// The `.x3b` artifact.
    #[arg(required = true)]
    pub artifact: PathBuf,

    /// The detached attestation (defaults to `<artifact>.sig.json`).
    #[arg(long)]
    pub attestation: Option<PathBuf>,

    /// The artifact key registry the signer must be in (`{"keys": [...]}`).
    #[arg(long, required = true)]
    pub registry: PathBuf,

    /// How the registry is trusted (`--registry-root` or `--unsigned-registry`).
    #[command(flatten)]
    pub registry_trust: crate::commands::registry_trust::RegistryTrust,
}

pub async fn execute(args: VerifyArtifactArgs) -> Result<()> {
    let artifact = std::fs::read(&args.artifact)
        .map_err(|e| CliError::Build(format!("read {}: {e}", args.artifact.display())))?;
    let sidecar = args
        .attestation
        .clone()
        .unwrap_or_else(|| attestation_path(&args.artifact));
    let body = std::fs::read_to_string(&sidecar)
        .map_err(|e| CliError::Build(format!("read {}: {e}", sidecar.display())))?;
    let attestation: x3_common::artifact::ArtifactAttestation = serde_json::from_str(&body)
        .map_err(|e| CliError::Build(format!("attestation {}: {e}", sidecar.display())))?;
    let registry = crate::commands::registry_trust::load_trusted_registry(
        &args.registry,
        &args.registry_trust,
    )?;

    attestation.verify(&artifact, &registry).map_err(|e| {
        CliError::Build(format!(
            "artifact {} does not verify: {e}",
            args.artifact.display()
        ))
    })?;
    let status = registry
        .status(&attestation.key_id)
        .map(|status| format!("{status:?}").to_lowercase())
        .unwrap_or_default();
    println!(
        "{} {} verified: signed by '{}' ({status})",
        "✓".green(),
        args.artifact.display(),
        attestation.key_id
    );
    Ok(())
}
