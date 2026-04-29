//! X3 AppZone factory.
//!
//! Provides template management, deployment validation, on-chain registry
//! tracking, and CLI argument parsing for the AppZone deployment workflow.
//!
//! # Modules
//!
//! * [`templates`] — template descriptors and the in-memory catalogue
//! * [`deploy`] — deployment request builder and commitment computation
//! * [`registry`] — zone lifecycle state machine
//! * [`cli`] — argument parser for the `x3-appzone` CLI binary
//!
//! # Scope note
//!
//! AppZone factory is **not** activated in the v0.4 minimal internal mainnet.
//! It ships as a library crate so the workflow can be developed and tested
//! without gating the mainnet RC, then enabled in a subsequent release once
//! the on-chain pallet integration is hardened.

#![cfg_attr(not(feature = "std"), no_std)]

extern crate alloc;

pub mod cli;
pub mod deploy;
pub mod registry;
pub mod templates;

pub use cli::{parse_args, CliError, Cmd};
pub use deploy::{DeployError, DeployRequest, Deployer};
pub use registry::{RegistryError, ZoneEntry, ZoneId, ZoneRegistry, ZoneStatus};
pub use templates::{Param, Template, TemplateCatalogue, TemplateError, TemplateId};

// ────────────────────────────────────────────────────────────────────────────
// Tests
// ────────────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_template() -> Template {
        Template {
            id: [0x01u8; 32],
            name: "defi-amm".into(),
            version: "0.4.0".into(),
            required_params: alloc::vec!["fee_bps".into(), "owner".into()],
            optional_params: alloc::vec![Param {
                key: "max_tvl".into(),
                value: "1000000".into(),
            }],
        }
    }

    // ── Template catalogue

    #[test]
    fn register_and_retrieve_template() {
        let mut cat = TemplateCatalogue::new();
        cat.register(sample_template()).unwrap();
        assert_eq!(cat.len(), 1);
        assert!(cat.get(&[0x01u8; 32]).is_some());
    }

    #[test]
    fn duplicate_template_registration_rejected() {
        let mut cat = TemplateCatalogue::new();
        cat.register(sample_template()).unwrap();
        assert_eq!(
            cat.register(sample_template()),
            Err(TemplateError::AlreadyExists)
        );
    }

    #[test]
    fn missing_required_param_detected() {
        let t = sample_template();
        // Only supply one of two required params.
        let params = alloc::vec![Param {
            key: "fee_bps".into(),
            value: "30".into(),
        }];
        assert_eq!(
            t.validate_params(&params),
            Err(TemplateError::MissingRequiredParam("owner".into()))
        );
    }

    #[test]
    fn all_required_params_accepted() {
        let t = sample_template();
        let params = alloc::vec![
            Param {
                key: "fee_bps".into(),
                value: "30".into()
            },
            Param {
                key: "owner".into(),
                value: "0xABCD".into()
            },
        ];
        assert!(t.validate_params(&params).is_ok());
    }

    // ── Deployer

    #[test]
    fn deploy_request_commitment_is_deterministic() {
        let params = alloc::vec![Param {
            key: "fee_bps".into(),
            value: "30".into(),
        }];
        let r1 = Deployer::build([0x01; 32], "zone-a".into(), params.clone()).unwrap();
        let r2 = Deployer::build([0x01; 32], "zone-a".into(), params).unwrap();
        assert_eq!(r1.commitment, r2.commitment);
    }

    #[test]
    fn deploy_request_commitment_is_input_sensitive() {
        let params = alloc::vec![];
        let r1 = Deployer::build([0x01; 32], "zone-a".into(), params.clone()).unwrap();
        let r2 = Deployer::build([0x02; 32], "zone-a".into(), params).unwrap();
        assert_ne!(r1.commitment, r2.commitment);
    }

    #[test]
    fn empty_zone_name_rejected() {
        assert_eq!(
            Deployer::build([0x01; 32], "".into(), alloc::vec![]),
            Err(DeployError::EmptyZoneName)
        );
    }

    #[test]
    fn zone_name_too_long_rejected() {
        let long_name = "a".repeat(65);
        assert_eq!(
            Deployer::build([0x01; 32], long_name, alloc::vec![]),
            Err(DeployError::ZoneNameTooLong)
        );
    }

    // ── Registry lifecycle

    #[test]
    fn zone_lifecycle_happy_path() {
        let mut reg = ZoneRegistry::new();
        let request = Deployer::build([0x01; 32], "zone-a".into(), alloc::vec![]).unwrap();
        let id = reg.register(request).unwrap();

        assert_eq!(reg.get(&id).unwrap().status, ZoneStatus::Pending);
        reg.activate(&id).unwrap();
        assert_eq!(reg.get(&id).unwrap().status, ZoneStatus::Active);
        reg.pause(&id).unwrap();
        assert_eq!(reg.get(&id).unwrap().status, ZoneStatus::Paused);
        reg.decommission(&id).unwrap();
        assert_eq!(reg.get(&id).unwrap().status, ZoneStatus::Decommissioned);
    }

    #[test]
    fn invalid_lifecycle_transition_rejected() {
        let mut reg = ZoneRegistry::new();
        let request = Deployer::build([0x02; 32], "zone-b".into(), alloc::vec![]).unwrap();
        let id = reg.register(request).unwrap();
        // Cannot decommission from Pending directly.
        assert!(matches!(
            reg.decommission(&id),
            Err(RegistryError::InvalidTransition { .. })
        ));
    }

    #[test]
    fn duplicate_registration_rejected() {
        let mut reg = ZoneRegistry::new();
        let request = Deployer::build([0x03; 32], "zone-c".into(), alloc::vec![]).unwrap();
        reg.register(request.clone()).unwrap();
        assert_eq!(reg.register(request), Err(RegistryError::AlreadyRegistered));
    }

    // ── CLI parser

    #[test]
    fn parse_list_templates() {
        assert_eq!(parse_args(&["list-templates"]), Ok(Cmd::ListTemplates));
    }

    #[test]
    fn parse_deploy_with_params() {
        let cmd = parse_args(&["deploy", "aabbcc", "my-zone", "fee_bps=30", "owner=0xAB"]);
        assert_eq!(
            cmd,
            Ok(Cmd::Deploy {
                template_id_hex: "aabbcc".into(),
                zone_name: "my-zone".into(),
                params: alloc::vec![
                    Param {
                        key: "fee_bps".into(),
                        value: "30".into()
                    },
                    Param {
                        key: "owner".into(),
                        value: "0xAB".into()
                    },
                ],
            })
        );
    }

    #[test]
    fn parse_status() {
        assert_eq!(
            parse_args(&["status", "deadbeef"]),
            Ok(Cmd::Status {
                zone_id_hex: "deadbeef".into()
            })
        );
    }

    #[test]
    fn parse_unknown_command_error() {
        assert!(matches!(
            parse_args(&["frobniculate"]),
            Err(CliError::UnknownCommand(_))
        ));
    }

    #[test]
    fn parse_empty_args_error() {
        assert_eq!(parse_args(&[]), Err(CliError::NoCommand));
    }
}
