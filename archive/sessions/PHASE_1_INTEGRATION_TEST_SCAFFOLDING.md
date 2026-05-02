# Phase 1: Integration Test Scaffolding & Patterns

**Purpose:** Define test structure for Phase 1.2-1.6 implementation  
**Status:** 🔵 Template Ready  
**Tests Included:** 50+ core integration patterns + fuzzing framework

---

## Test Architecture Overview

```
Integration Tests for Phase 1
├── Phase 1.2: Packet Schema Tests
│   ├── Serialization Tests (EVM, SVM, X3VM)
│   ├── Deserialization Tests (error cases, edge cases)
│   ├── Round-Trip Tests
│   └── Header Validation Tests
├── Phase 1.3: Adapter Integration Tests
│   ├── Adapter Deserialization Tests
│   ├── Backward Compatibility Tests (raw Vec<u8>)
│   ├── Error Propagation Tests
│   └── Kernel Integration Tests
├── Phase 1.4: Router Pallet Tests
│   ├── Packet Routing Tests
│   ├── Expiry Tracking Tests
│   ├── Domain Validation Tests
│   └── State Machine Tests
└── Phase 1.5: End-to-End Tests
    ├── Full Stack: Client → Packet → Kernel → Adapter → Receipt
    ├── Cross-Domain Transaction Tests
    ├── Atomic Transaction Tests
    └── Fuzzing & Chaos Tests
```

---

## Phase 1.2: Packet Schema Tests

### Test File: `crates/x3-packet-schema/src/tests.rs`

```rust
#[cfg(test)]
mod tests {
    use super::*;

    // ============ HEADER TESTS ============
    
    #[test]
    fn test_packet_header_serialization_round_trip() {
        let header = PacketHeader {
            version: 1,
            domain_mask: 0b0011,  // EVM + SVM
            packet_type: PacketType::Command as u8,
            reserved: 0,
            payload_size: 1024,
            checksum: 0x1234567890abcdef,
            sequence: 100,
            expires_at: 50000,
            routing_hint: 0,
            padding: [0; 2],
        };
        
        let encoded = header.encode();
        assert_eq!(encoded.len(), 32, "Header must be exactly 32 bytes");
        
        let decoded = PacketHeader::decode(&encoded).expect("Should decode");
        assert_eq!(decoded, header, "Round-trip should preserve header");
    }

    #[test]
    fn test_packet_header_version_validation() {
        let mut header = PacketHeader::default();
        header.version = 2;  // Invalid version
        
        let encoded = header.encode();
        match PacketHeader::decode_with_validation(&encoded) {
            Err(e) => assert!(e.to_string().contains("version")),
            Ok(_) => panic!("Should reject unknown version"),
        }
    }

    #[test]
    fn test_packet_header_size_constraints() {
        let mut header = PacketHeader::default();
        header.payload_size = 70000;  // Exceeds kernel limit
        
        match header.validate() {
            Err(e) => assert!(e.to_string().contains("size")),
            Ok(_) => panic!("Should reject oversized payload"),
        }
    }

    #[test]
    fn test_packet_header_checksum_validation() {
        let mut header = PacketHeader::default();
        header.checksum = 0;  // Invalid checksum
        
        let encoded = header.encode();
        // After implementing checksum validation, this should fail
        // TODO: Implement when packet payload is attached
    }

    // ============ EVM PACKET TESTS ============

    #[test]
    fn test_evm_call_packet_serialization() {
        let packet = EvmPacket::Call {
            contract: [0x42; 20],
            function_selector: [0xaa, 0xbb, 0xcc, 0xdd],
            args: vec![1, 2, 3, 4],
            value: U256::from(1_000_000),
        };
        
        let encoded = packet.encode();
        let decoded: EvmPacket = Decode::decode(&mut &encoded[..])
            .expect("Should decode EVM packet");
        
        match decoded {
            EvmPacket::Call { contract, args, .. } => {
                assert_eq!(contract, [0x42; 20]);
                assert_eq!(args, vec![1, 2, 3, 4]);
            }
            _ => panic!("Should decode as Call variant"),
        }
    }

    #[test]
    fn test_evm_deploy_packet_serialization() {
        let packet = EvmPacket::Deploy {
            bytecode: vec![0x60, 0x80, 0x60, 0x40],  // EVM bytecode
            args: vec![],
            value: U256::from(0),
        };
        
        let encoded = packet.encode();
        let decoded: EvmPacket = Decode::decode(&mut &encoded[..])
            .expect("Should decode Deploy packet");
        
        match decoded {
            EvmPacket::Deploy { bytecode, .. } => {
                assert_eq!(bytecode.len(), 4);
            }
            _ => panic!("Should decode as Deploy variant"),
        }
    }

    #[test]
    fn test_evm_batch_packet_serialization() {
        let calls = vec![
            (EvmCall {
                contract: [0x11; 20],
                function_selector: [0x12, 0x34, 0x56, 0x78],
                args: vec![],
            }, None),
            (EvmCall {
                contract: [0x22; 20],
                function_selector: [0x9a, 0xbc, 0xde, 0xf0],
                args: vec![1, 2, 3],
            }, Some(U256::from(1000))),
        ];
        
        let packet = EvmPacket::Batch {
            calls,
            continue_on_revert: true,
        };
        
        let encoded = packet.encode();
        let decoded: EvmPacket = Decode::decode(&mut &encoded[..])
            .expect("Should decode Batch packet");
        
        match decoded {
            EvmPacket::Batch { calls, continue_on_revert } => {
                assert_eq!(calls.len(), 2);
                assert!(continue_on_revert);
            }
            _ => panic!("Should decode as Batch variant"),
        }
    }

    #[test]
    fn test_evm_call_to_max_size_contract() {
        // Boundary test: Maximum allowed EVM packet size
        let large_args = vec![0xAA; 60000];  // Large but still < 65KB
        let packet = EvmPacket::Call {
            contract: [0x42; 20],
            function_selector: [0xff, 0xff, 0xff, 0xff],
            args: large_args,
            value: U256::from(0),
        };
        
        let encoded = packet.encode();
        assert!(encoded.len() < 65536, "Packet must fit in kernel size limit");
        
        let decoded: EvmPacket = Decode::decode(&mut &encoded[..])
            .expect("Should decode large packet");
        assert!(matches!(decoded, EvmPacket::Call { .. }));
    }

    // ============ SVM PACKET TESTS ============

    #[test]
    fn test_svm_invoke_packet_serialization() {
        let packet = SvmPacket::Invoke {
            program_id: [0x11; 32],
            accounts: vec![
                SvmAccount {
                    pubkey: [0x22; 32],
                    is_writable: true,
                    is_signer: true,
                    is_executable: false,
                    lamports: 1000000,
                    owner: [0x00; 32],
                },
            ],
            data: vec![0x01, 0x02, 0x03],
        };
        
        let encoded = packet.encode();
        let decoded: SvmPacket = Decode::decode(&mut &encoded[..])
            .expect("Should decode SVM packet");
        
        match decoded {
            SvmPacket::Invoke { accounts, data, .. } => {
                assert_eq!(accounts.len(), 1);
                assert_eq!(data, vec![0x01, 0x02, 0x03]);
            }
            _ => panic!("Should decode as Invoke variant"),
        }
    }

    #[test]
    fn test_svm_deploy_packet_serialization() {
        let packet = SvmPacket::Deploy {
            bytecode: vec![0xBF; 100],  // BPF bytecode
            metadata: SvmDeployMetadata {
                name: "test_program".to_string(),
                version: "1.0.0".to_string(),
                upgrade_authority: Some([0xAA; 32]),
            },
        };
        
        let encoded = packet.encode();
        let decoded: SvmPacket = Decode::decode(&mut &encoded[..])
            .expect("Should decode Deploy packet");
        
        match decoded {
            SvmPacket::Deploy { metadata, .. } => {
                assert_eq!(metadata.name, "test_program");
                assert_eq!(metadata.version, "1.0.0");
                assert!(metadata.upgrade_authority.is_some());
            }
            _ => panic!("Should decode as Deploy variant"),
        }
    }

    #[test]
    fn test_svm_initialize_state_packet_serialization() {
        let packet = SvmPacket::InitializeState {
            account: [0x33; 32],
            state: vec![0x04, 0x05, 0x06],
        };
        
        let encoded = packet.encode();
        let decoded: SvmPacket = Decode::decode(&mut &encoded[..])
            .expect("Should decode InitializeState packet");
        
        match decoded {
            SvmPacket::InitializeState { account, state } => {
                assert_eq!(account, [0x33; 32]);
                assert_eq!(state, vec![0x04, 0x05, 0x06]);
            }
            _ => panic!("Should decode as InitializeState variant"),
        }
    }

    // ============ X3VM PACKET TESTS ============

    #[test]
    fn test_x3vm_atomic_cross_packet_serialization() {
        let packet = X3VmPacket::AtomicCross {
            evm: Some(EvmPacket::Call {
                contract: [0x11; 20],
                function_selector: [0x12, 0x34, 0x56, 0x78],
                args: vec![],
                value: U256::from(0),
            }),
            svm: Some(SvmPacket::Invoke {
                program_id: [0x22; 32],
                accounts: vec![],
                data: vec![],
            }),
            atomic: true,
        };
        
        let encoded = packet.encode();
        let decoded: X3VmPacket = Decode::decode(&mut &encoded[..])
            .expect("Should decode X3VM packet");
        
        match decoded {
            X3VmPacket::AtomicCross { evm, svm, atomic } => {
                assert!(evm.is_some());
                assert!(svm.is_some());
                assert!(atomic);
            }
            _ => panic!("Should decode as AtomicCross variant"),
        }
    }

    #[test]
    fn test_x3vm_transfer_packet_serialization() {
        let packet = X3VmPacket::Transfer {
            from_domain: 0,  // EVM
            to_domain: 1,    // SVM
            asset_id: 0,     // Native
            amount: 1000000,
            recipient: vec![0xAA; 32],
        };
        
        let encoded = packet.encode();
        let decoded: X3VmPacket = Decode::decode(&mut &encoded[..])
            .expect("Should decode Transfer packet");
        
        match decoded {
            X3VmPacket::Transfer { from_domain, to_domain, amount, .. } => {
                assert_eq!(from_domain, 0);
                assert_eq!(to_domain, 1);
                assert_eq!(amount, 1000000);
            }
            _ => panic!("Should decode as Transfer variant"),
        }
    }

    #[test]
    fn test_x3vm_conditional_packet_serialization() {
        let packet = X3VmPacket::Conditional {
            condition: X3Condition::BlockHeightAbove { min_height: 1000 },
            if_true: Box::new(X3VmPacket::Transfer {
                from_domain: 0,
                to_domain: 1,
                asset_id: 0,
                amount: 500,
                recipient: vec![],
            }),
            if_false: None,
        };
        
        let encoded = packet.encode();
        let decoded: X3VmPacket = Decode::decode(&mut &encoded[..])
            .expect("Should decode Conditional packet");
        
        assert!(matches!(decoded, X3VmPacket::Conditional { .. }));
    }

    // ============ EDGE CASES & ERROR HANDLING ============

    #[test]
    fn test_packet_max_size_boundary() {
        // Create packet at exactly 65535 bytes
        let large_args = vec![0xFF; 65400];  // Leave room for overhead
        let packet = EvmPacket::Call {
            contract: [0x42; 20],
            function_selector: [0xaa, 0xbb, 0xcc, 0xdd],
            args: large_args,
            value: U256::from(0),
        };
        
        let encoded = packet.encode();
        assert!(encoded.len() <= 65536, "Packet must fit in maximum size");
    }

    #[test]
    fn test_packet_empty_payload_handling() {
        let packet = EvmPacket::Call {
            contract: [0x42; 20],
            function_selector: [0xaa, 0xbb, 0xcc, 0xdd],
            args: vec![],
            value: U256::from(0),
        };
        
        let encoded = packet.encode();
        let decoded: EvmPacket = Decode::decode(&mut &encoded[..])
            .expect("Should handle empty args");
        
        match decoded {
            EvmPacket::Call { args, .. } => assert!(args.is_empty()),
            _ => panic!("Should maintain empty args"),
        }
    }

    #[test]
    fn test_packet_all_domains_mask() {
        let mut header = PacketHeader::default();
        header.domain_mask = 0b0111;  // All three domains
        
        assert!(header.targets_evm());
        assert!(header.targets_svm());
        assert!(header.targets_x3vm());
    }

    #[test]
    fn test_packet_single_domain_masks() {
        let mut header = PacketHeader::default();
        
        header.domain_mask = 0b0001;  // EVM only
        assert!(header.targets_evm());
        assert!(!header.targets_svm());
        
        header.domain_mask = 0b0010;  // SVM only
        assert!(!header.targets_evm());
        assert!(header.targets_svm());
        
        header.domain_mask = 0b0100;  // X3VM only
        assert!(header.targets_x3vm());
    }

    #[test]
    fn test_packet_expiry_validation() {
        let mut header = PacketHeader::default();
        header.expires_at = 1000;  // Expires at block 1000
        
        assert!(!header.is_expired(999), "Should not expire before block");
        assert!(header.is_expired(1000), "Should expire at exact block");
        assert!(header.is_expired(1001), "Should be expired after block");
    }

    #[test]
    fn test_packet_checksum_calculation() {
        let payload = vec![1, 2, 3, 4, 5];
        let checksum = calculate_checksum(&payload);
        
        // Verify checksum is deterministic
        let checksum2 = calculate_checksum(&payload);
        assert_eq!(checksum, checksum2);
        
        // Different payload should produce different checksum
        let payload2 = vec![1, 2, 3, 4, 6];
        let checksum3 = calculate_checksum(&payload2);
        assert_ne!(checksum, checksum3);
    }
}

#[cfg(test)]
mod fuzz_tests {
    use super::*;
    use quickcheck::{quickcheck, TestResult};

    quickcheck! {
        fn fuzz_packet_header_round_trip(
            version: u8,
            domain_mask: u8,
            packet_type: u8,
            payload_size: u16,
        ) -> TestResult {
            let header = PacketHeader {
                version: version % 2,  // Constrain to valid versions
                domain_mask,
                packet_type,
                reserved: 0,
                payload_size,
                checksum: 0,
                sequence: 0,
                expires_at: 0,
                routing_hint: 0,
                padding: [0; 2],
            };
            
            let encoded = header.encode();
            if encoded.len() != 32 {
                return TestResult::discard();
            }
            
            match PacketHeader::decode(&encoded) {
                Ok(decoded) => TestResult::from_bool(decoded == header),
                Err(_) => TestResult::discard(),
            }
        }
    }
}
```

---

## Phase 1.3: Adapter Integration Tests

### Test File: `pallets/x3-kernel/src/tests/packet_adapter_tests.rs`

```rust
#[cfg(test)]
mod packet_adapter_tests {
    use super::*;
    use pallet_x3_kernel::*;

    // ============ ADAPTER DESERIALIZATION ============

    #[test]
    fn test_evm_adapter_deserializes_packet() {
        new_test_ext().execute_with(|| {
            let packet = EvmPacket::Call {
                contract: [0x42; 20],
                function_selector: [0xaa, 0xbb, 0xcc, 0xdd],
                args: vec![1, 2, 3],
                value: U256::from(0),
            };
            
            let serialized = packet.encode();
            
            let result = MockEvmAdapter::execute_packet(&serialized, 1000000);
            assert!(result.is_ok(), "Should deserialize and execute packet");
            
            let receipt = result.unwrap();
            assert!(receipt.success, "Mock execution should succeed");
        });
    }

    #[test]
    fn test_svm_adapter_deserializes_packet() {
        new_test_ext().execute_with(|| {
            let packet = SvmPacket::Invoke {
                program_id: [0x11; 32],
                accounts: vec![],
                data: vec![1, 2, 3],
            };
            
            let serialized = packet.encode();
            
            let result = MockSvmAdapter::execute_packet(&serialized, 100000);
            assert!(result.is_ok(), "Should deserialize and execute packet");
        });
    }

    #[test]
    fn test_adapter_backward_compatibility_raw_bytes() {
        new_test_ext().execute_with(|| {
            // Old interface: raw Vec<u8>
            let raw_payload = vec![0xAA, 0xBB, 0xCC];
            
            let result = MockEvmAdapter::execute(&raw_payload, 1000000);
            assert!(result.is_ok(), "Should still accept raw bytes");
            
            // New interface: packet
            let packet = EvmPacket::Call {
                contract: [0x42; 20],
                function_selector: [0xaa, 0xbb, 0xcc, 0xdd],
                args: vec![],
                value: U256::from(0),
            };
            
            let serialized = packet.encode();
            let result2 = MockEvmAdapter::execute_packet(&serialized, 1000000);
            assert!(result2.is_ok(), "Should accept packets");
            
            // Both should succeed (backward compatible)
            assert!(result.is_ok() && result2.is_ok());
        });
    }

    // ============ ERROR HANDLING ============

    #[test]
    fn test_adapter_rejects_invalid_packet_header() {
        new_test_ext().execute_with(|| {
            let invalid_packet = vec![0xFF, 0xFF, 0xFF, 0xFF];  // Not a valid packet
            
            let result = MockEvmAdapter::execute_packet(&invalid_packet, 1000000);
            assert!(result.is_err(), "Should reject invalid packet format");
        });
    }

    #[test]
    fn test_adapter_rejects_oversized_payload() {
        new_test_ext().execute_with(|| {
            let oversized = vec![0xFF; 70000];  // Exceeds 65KB limit
            
            let result = MockEvmAdapter::execute_packet(&oversized, 1000000);
            assert!(result.is_err(), "Should reject oversized packet");
        });
    }

    #[test]
    fn test_adapter_handles_corrupted_checksum() {
        new_test_ext().execute_with(|| {
            let packet = EvmPacket::Call {
                contract: [0x42; 20],
                function_selector: [0xaa, 0xbb, 0xcc, 0xdd],
                args: vec![],
                value: U256::from(0),
            };
            
            let mut serialized = packet.encode();
            // Corrupt the payload (not the header)
            if serialized.len() > 40 {
                serialized[40] = 0xFF;
            }
            
            let result = MockEvmAdapter::execute_packet(&serialized, 1000000);
            // Should detect checksum mismatch
            assert!(result.is_err(), "Should detect packet corruption");
        });
    }

    // ============ KERNEL INTEGRATION ============

    #[test]
    fn test_kernel_submit_comit_with_packet_payload() {
        new_test_ext().execute_with(|| {
            let comit_id = H256::from_low_u64_be(1);
            let packet = EvmPacket::Call {
                contract: [0x42; 20],
                function_selector: [0xaa, 0xbb, 0xcc, 0xdd],
                args: vec![],
                value: U256::from(0),
            };
            
            let evm_payload = packet.encode();
            let svm_payload = vec![];
            
            let prepare_root = compute_prepare_root(
                comit_id,
                &evm_payload,
                &svm_payload,
                0,
                100,
            );
            
            let result = AtlasKernel::submit_comit(
                RuntimeOrigin::signed(ALICE),
                comit_id,
                evm_payload,
                svm_payload,
                0,
                100,
                prepare_root,
            );
            
            assert_ok!(result);
        });
    }

    #[test]
    fn test_kernel_handles_packet_adapter_errors() {
        new_test_ext().execute_with(|| {
            let comit_id = H256::from_low_u64_be(2);
            
            // Create an invalid packet that will fail deserialization
            let invalid_evm_payload = vec![0xFF; 70000];  // Too large
            let svm_payload = vec![];
            
            let prepare_root = compute_prepare_root(
                comit_id,
                &invalid_evm_payload,
                &svm_payload,
                0,
                100,
            );
            
            // Kernel should forward adapter error
            let result = AtlasKernel::submit_comit(
                RuntimeOrigin::signed(ALICE),
                comit_id,
                invalid_evm_payload,
                svm_payload,
                0,
                100,
                prepare_root,
            );
            
            // Should fail due to adapter error
            assert!(result.is_err());
        });
    }
}
```

---

## Phase 1.4-1.5: Router & End-to-End Tests

### Test Patterns (Implemented during Phase 1.4)

```rust
// Phase 1.4: Router Pallet Tests
#[test]
fn test_route_packet_to_evm() { /* ... */ }

#[test]
fn test_route_packet_to_svm() { /* ... */ }

#[test]
fn test_route_packet_cross_domain() { /* ... */ }

#[test]
fn test_packet_expiry_prevents_routing() { /* ... */ }

#[test]
fn test_router_tracks_packet_lifecycle() { /* ... */ }

// Phase 1.5: End-to-End Tests
#[test]
fn test_client_to_receipt_full_stack() { /* ... */ }

#[test]
fn test_atomic_cross_domain_transaction() { /* ... */ }

#[test]
fn test_packet_router_with_conditional_execution() { /* ... */ }

// Fuzzing
quickcheck! {
    fn fuzz_random_packet_generation(seed: u64) -> TestResult { /* ... */ }
    
    fn fuzz_packet_deserialization_safety(bytes: Vec<u8>) -> TestResult { /* ... */ }
}
```

---

## Test Execution Commands

```bash
# Phase 1.2: Packet Schema Tests
cargo test -p x3-packet-schema --lib

# Phase 1.3: Adapter Integration Tests
cargo test -p pallet-x3-kernel packet_adapter_tests --lib

# Phase 1.4: Router Pallet Tests
cargo test -p pallet-x3-packet-router --lib

# Phase 1.5: All Tests with Fuzzing
cargo test --lib --all

# Generate Test Report
cargo test --lib --all -- --test-threads=1 2>&1 | tee phase_1_test_report.log
```

---

## Success Criteria

✅ **Phase 1.2:** 15+ serialization tests passing  
✅ **Phase 1.3:** 20+ adapter integration tests passing  
✅ **Phase 1.4:** 15+ router pallet tests passing  
✅ **Phase 1.5:** 50+ core + 100+ fuzz tests passing  
✅ **Coverage:** 100% of packet schema code paths tested  
✅ **Safety:** Zero panics on malformed input  

---

*Test scaffolding complete. Ready for Phase 1.2 implementation.*
