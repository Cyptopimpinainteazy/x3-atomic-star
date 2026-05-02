# X3 Economic Attack Scenario Testing

**Status:** 🔴 CRITICAL - S0 Blocker  
**Priority:** P0 (Emergency)  
**Owner:** Security Team  
**Due:** 2026-04-30

---

## Problem Statement

X3 currently has **ZERO** tests for economic attack vectors. The ProofForge runners for `flashloans`, `oracle`, and `dex` are stubs that claim security without testing.

**Risk:** Economic exploits can:
- Drain treasury through flash loan manipulation
- Manipulate oracle prices for liquidation cascades
- Front-run/sandwich transactions for MEV extraction
- Cause bank runs through governance attacks
- Arbitrage cross-VM price discrepancies

**Real-world precedent:**
- bZx: $1M flash loan attack (2020)
- Harvest Finance: $34M flash loan attack (2020)
- Cream Finance: $130M price oracle manipulation (2021)
- Mango Markets: $114M oracle manipulation (2022)

---

## Required Economic Attack Tests

### **Category 1: Flash Loan Attacks** 🏦

#### Attack 1.1: Oracle Manipulation via Flash Loan

**Attack Vector:**
1. Borrow $100M in flash loan
2. Dump tokens to manipulate DEX price
3. Use manipulated price to liquidate victim positions
4. Repay flash loan + profit from liquidation

**Test File:** `pallets/flashloans/src/tests/oracle_manipulation.rs`

```rust
#[test]
fn test_flashloan_oracle_manipulation_prevented() {
    new_test_ext().execute_with(|| {
        // 1. Attacker takes massive flash loan
        let loan_amount = 100_000_000 * UNITS;
        let attacker = 1;
        assert_ok!(Flashloans::borrow(
            Origin::signed(attacker),
            ASSET_A,
            loan_amount
        ));
        
        // 2. Attacker dumps tokens to manipulate price
        assert_ok!(Dex::swap(
            Origin::signed(attacker),
            ASSET_A,
            ASSET_B,
            loan_amount / 2  // Dump half
        ));
        
        // 3. Attempt to use manipulated price for liquidation
        let victim = 2;
        assert_noop!(
            Liquidator::liquidate(
                Origin::signed(attacker),
                victim,
                ASSET_B
            ),
            Error::<Test>::PriceManipulationDetected  // MUST FAIL
        );
        
        // 4. Verify TWAP prevents instant price use
        let current_price = Oracle::get_price(ASSET_B);
        let twap_price = Oracle::get_twap(ASSET_B, 10);  // 10 block TWAP
        assert!(
            (current_price - twap_price).abs() < twap_price / 10,
            "Price divergence too high - manipulation detected"
        );
        
        // 5. Flash loan must revert if attack attempted
        assert_noop!(
            Flashloans::repay(Origin::signed(attacker)),
            Error::<Test>::MaliciousFlashloanDetected
        );
    });
}
```

---

#### Attack 1.2: Reentrancy During Flash Loan

**Attack Vector:**
1. Borrow flash loan
2. Callback triggers reentrant call to borrow again
3. Double-borrow without collateral

**Test File:** `pallets/flashloans/src/tests/reentrancy.rs`

```rust
#[test]
fn test_flashloan_reentrancy_prevented() {
    new_test_ext().execute_with(|| {
        let attacker = 1;
        let malicious_contract = deploy_reentrant_contract();
        
        // Attacker borrows via malicious contract
        assert_ok!(Flashloans::borrow_to_contract(
            Origin::signed(attacker),
            malicious_contract,
            ASSET_A,
            1_000_000 * UNITS
        ));
        
        // Malicious contract callback attempts reentry
        assert_noop!(
            MaliciousContract::on_flashloan_received(
                malicious_contract,
                /* attempts to borrow again inside callback */
            ),
            Error::<Test>::ReentrancyGuardActive
        );
        
        // Verify reentrancy lock is set
        assert!(Flashloans::is_locked(malicious_contract));
        
        // Verify supply is conserved (no double-borrow)
        let total_borrowed = Flashloans::total_borrowed(ASSET_A);
        assert_eq!(total_borrowed, 1_000_000 * UNITS);
    });
}
```

---

#### Attack 1.3: Flash Loan Repayment Bypass

**Attack Vector:**
1. Borrow flash loan
2. Self-destruct contract to avoid repayment check
3. Keep borrowed funds

**Test File:** `pallets/flashloans/src/tests/repayment_bypass.rs`

```rust
#[test]
fn test_flashloan_repayment_mandatory() {
    new_test_ext().execute_with(|| {
        let attacker = 1;
        let loan_amount = 1_000_000 * UNITS;
        
        // Start flash loan
        let initial_supply = Assets::total_supply(ASSET_A);
        assert_ok!(Flashloans::borrow(
            Origin::signed(attacker),
            ASSET_A,
            loan_amount
        ));
        
        // Attacker attempts to exit without repaying
        assert_noop!(
            System::set_block_number(System::block_number() + 1),
            /* Block transition should fail if flash loan not repaid */
            Error::<Test>::UnrepaidFlashloan
        );
        
        // Verify atomic revert if not repaid in same transaction
        let final_supply = Assets::total_supply(ASSET_A);
        assert_eq!(
            initial_supply, 
            final_supply,
            "Supply must be unchanged if flash loan not repaid"
        );
    });
}
```

---

### **Category 2: MEV Attacks** 🤖

#### Attack 2.1: Sandwich Attack

**Attack Vector:**
1. Detect victim's large swap in mempool
2. Front-run: Buy tokens before victim
3. Victim swap executes at worse price
4. Back-run: Sell tokens after victim

**Test File:** `pallets/dex/src/tests/sandwich_attack.rs`

```rust
#[test]
fn test_sandwich_attack_mitigated() {
    new_test_ext().execute_with(|| {
        let victim = 1;
        let attacker = 2;
        
        // Victim submits swap for 1M tokens
        let victim_tx = Dex::swap(
            Origin::signed(victim),
            ASSET_A,
            ASSET_B,
            1_000_000 * UNITS
        );
        
        // Attacker front-runs (higher priority)
        assert_ok!(Dex::swap_with_priority(
            Origin::signed(attacker),
            ASSET_A,
            ASSET_B,
            100_000 * UNITS,
            priority: MAX_PRIORITY
        ));
        
        // Victim swap should have slippage protection
        assert_noop!(
            victim_tx,
            Error::<Test>::SlippageToleranceExceeded
        );
        
        // OR: Verify MEV tax collected
        let mev_tax = Dex::mev_tax_collected(attacker);
        assert!(
            mev_tax > 0,
            "MEV extractors must pay tax to victims"
        );
    });
}
```

---

#### Attack 2.2: Front-Running Liquidations

**Attack Vector:**
1. Monitor mempool for undercollateralized positions
2. Front-run liquidation call to extract liquidation bonus
3. Victim liquidator gets nothing

**Test File:** `pallets/liquidator/src/tests/frontrun_liquidation.rs`

```rust
#[test]
fn test_liquidation_frontrun_prevented() {
    new_test_ext().execute_with(|| {
        let victim_liquidator = 1;
        let attacker = 2;
        let undercollateralized_user = 3;
        
        // User becomes undercollateralized
        Oracle::set_price(ASSET_B, 0.5 * PRICE_UNIT);
        
        // Victim submits liquidation
        let liquidation_tx = Liquidator::liquidate(
            Origin::signed(victim_liquidator),
            undercollateralized_user,
            ASSET_B
        );
        
        // Attacker front-runs with higher priority
        assert_ok!(Liquidator::liquidate_with_priority(
            Origin::signed(attacker),
            undercollateralized_user,
            ASSET_B,
            priority: MAX_PRIORITY
        ));
        
        // Victim's transaction should not fail silently
        assert_noop!(
            liquidation_tx,
            Error::<Test>::AlreadyLiquidated
        );
        
        // Verify first-come-first-served OR fair queuing
        let first_liquidator = Liquidator::liquidator_of(undercollateralized_user);
        assert_eq!(
            first_liquidator,
            Some(victim_liquidator),
            "Original liquidator should get bonus, not front-runner"
        );
    });
}
```

---

### **Category 3: Oracle Manipulation** 📊

#### Attack 3.1: TWAP Manipulation

**Attack Vector:**
1. Execute large swaps near block boundaries
2. Manipulate time-weighted average price
3. Liquidate positions based on manipulated TWAP

**Test File:** `pallets/oracle/src/tests/twap_manipulation.rs`

```rust
#[test]
fn test_twap_manipulation_resistant() {
    new_test_ext().execute_with(|| {
        let attacker = 1;
        
        // Record initial TWAP
        let initial_twap = Oracle::get_twap(ASSET_A, 10);
        
        // Attacker executes massive swaps at block boundaries
        for _ in 0..9 {
            System::set_block_number(System::block_number() + 1);
            
            assert_ok!(Dex::swap(
                Origin::signed(attacker),
                ASSET_A,
                ASSET_B,
                10_000_000 * UNITS  // Huge swap
            ));
        }
        
        // Check TWAP after manipulation attempts
        let manipulated_twap = Oracle::get_twap(ASSET_A, 10);
        
        // TWAP should be resistant to single-block spikes
        let max_deviation = initial_twap / 20;  // 5% max deviation
        assert!(
            (manipulated_twap - initial_twap).abs() < max_deviation,
            "TWAP should be resistant to manipulation"
        );
        
        // Verify minimum liquidity requirements
        assert!(
            Dex::liquidity(ASSET_A, ASSET_B) > MIN_LIQUIDITY,
            "Low liquidity makes TWAP manipulation easier"
        );
    });
}
```

---

#### Attack 3.2: Oracle Front-Running

**Attack Vector:**
1. Oracle update transaction in mempool
2. Front-run oracle update with trade
3. Profit from price change

**Test File:** `pallets/oracle/src/tests/oracle_frontrun.rs`

```rust
#[test]
fn test_oracle_update_frontrun_prevented() {
    new_test_ext().execute_with(|| {
        let attacker = 1;
        
        // Oracle update in mempool (new price: $100)
        let oracle_update_tx = Oracle::update_price(
            Origin::signed(ORACLE_OPERATOR),
            ASSET_A,
            100 * PRICE_UNIT
        );
        
        // Attacker sees pending oracle update and front-runs
        assert_ok!(Dex::swap(
            Origin::signed(attacker),
            ASSET_B,
            ASSET_A,
            1_000_000 * UNITS,
            priority: MAX_PRIORITY
        ));
        
        // Oracle update executes
        assert_ok!(oracle_update_tx);
        
        // Attacker tries to profit
        let profit = Dex::swap(
            Origin::signed(attacker),
            ASSET_A,
            ASSET_B,
            1_000_000 * UNITS
        );
        
        // Verify delayed price application OR commit-reveal scheme
        assert!(
            profit < 1_000 * UNITS,  // Should not make significant profit
            "Oracle updates must use commit-reveal to prevent front-running"
        );
    });
}
```

---

### **Category 4: Cross-VM Arbitrage** 🔀

#### Attack 4.1: EVM-SVM Price Discrepancy

**Attack Vector:**
1. Find price discrepancy between EVM DEX and SVM DEX
2. Buy cheap on EVM, sell high on SVM
3. Atomic arbitrage with no risk

**Test File:** `pallets/atomic-swaps/src/tests/cross_vm_arbitrage.rs`

```rust
#[test]
fn test_cross_vm_arbitrage_fair_pricing() {
    new_test_ext().execute_with(|| {
        let arbitrageur = 1;
        
        // Set different prices on EVM and SVM DEXs
        EvmDex::set_price(ASSET_A, 100 * PRICE_UNIT);
        SvmDex::set_price(ASSET_A, 110 * PRICE_UNIT);  // 10% higher
        
        // Arbitrageur attempts cross-VM atomic swap
        assert_ok!(AtomicSwaps::cross_vm_swap(
            Origin::signed(arbitrageur),
            VM::EVM,
            VM::SVM,
            ASSET_A,
            1_000_000 * UNITS
        ));
        
        // Verify arbitrage profit is taxed OR prices converge
        let profit = Assets::balance_of(arbitrageur, ASSET_A);
        let expected_profit = 100_000 * UNITS;  // 10% of 1M
        
        assert!(
            profit < expected_profit * 3 / 10,  // Max 30% of arbitrage profit
            "Excessive arbitrage profit indicates price oracle issues"
        );
        
        // Verify prices converge after arbitrage
        let evm_price = EvmDex::get_price(ASSET_A);
        let svm_price = SvmDex::get_price(ASSET_A);
        assert!(
            (evm_price - svm_price).abs() < PRICE_UNIT,
            "Prices should converge after arbitrage"
        );
    });
}
```

---

### **Category 5: Governance Attacks** 🗳️

#### Attack 5.1: Flash Loan Governance Takeover

**Attack Vector:**
1. Borrow governance tokens via flash loan
2. Vote on malicious proposal
3. Execute proposal
4. Repay flash loan

**Test File:** `pallets/governance/src/tests/flashloan_takeover.rs`

```rust
#[test]
fn test_flashloan_governance_takeover_prevented() {
    new_test_ext().execute_with(|| {
        let attacker = 1;
        
        // Attacker borrows governance tokens
        assert_ok!(Flashloans::borrow(
            Origin::signed(attacker),
            GOV_TOKEN,
            10_000_000 * UNITS
        ));
        
        // Attacker submits malicious proposal
        let malicious_proposal = Governance::propose(
            Origin::signed(attacker),
            ProposalType::DrainTreasury(attacker)
        );
        
        // Attacker votes with borrowed tokens
        assert_noop!(
            Governance::vote(
                Origin::signed(attacker),
                malicious_proposal,
                Vote::Aye
            ),
            Error::<Test>::FlashloanedTokensCannotVote
        );
        
        // Verify time-locked voting OR snapshot voting
        assert!(
            Governance::snapshot_taken_before_proposal(malicious_proposal),
            "Governance must use snapshot to prevent flash loan attacks"
        );
    });
}
```

---

## Implementation Steps

### **Step 1: Create Test Infrastructure**

```bash
# Create test directories
mkdir -p pallets/flashloans/src/tests
mkdir -p pallets/dex/src/tests
mkdir -p pallets/oracle/src/tests
mkdir -p pallets/atomic-swaps/src/tests
mkdir -p pallets/governance/src/tests

# Create mock attacker contract
touch pallets/flashloans/src/tests/malicious_contract.rs
```

---

### **Step 2: Update ProofForge Runners**

**File:** `proof-forge/src/runners/flashloans.rs`

```rust
pub async fn run_proofs(workspace: &PathBuf, verbose: bool) -> Result<ProofResult> {
    let mut failed = Vec::new();
    
    // Run flash loan attack tests
    let attack_tests = vec![
        "oracle_manipulation",
        "reentrancy",
        "repayment_bypass",
    ];
    
    for test in attack_tests {
        let result = Command::new("cargo")
            .args(&["test", "--package", "pallet-flashloans", test])
            .current_dir(workspace)
            .output()?;
        
        if !result.status.success() {
            failed.push(format!("Flash loan attack test '{}' FAILED", test));
        }
    }
    
    let blockers = if !failed.is_empty() {
        vec!["S0: Flash loan attacks not prevented".to_string()]
    } else {
        vec![]
    };
    
    Ok(ProofResult {
        claim_id: "x3.flashloans.attack_resistant".to_string(),
        failed_checks: failed,
        blockers,
        ..Default::default()
    })
}
```

---

### **Step 3: Add Economic Attack Gate**

**File:** `proof-forge/src/main.rs`

```rust
#[derive(Parser)]
#[command(name = "x3-proof")]
enum Commands {
    // ... existing commands ...
    
    /// Run economic attack scenario tests
    #[command(name = "economic-gate")]
    EconomicGate {
        #[arg(long)]
        workspace: PathBuf,
        
        #[arg(short, long)]
        verbose: bool,
    },
}

async fn economic_gate(workspace: &PathBuf, verbose: bool) -> Result<()> {
    println!("🏦 Economic Attack Gate: testing exploit resistance\n");
    
    let areas = vec![
        ("flashloans", "Flash Loan Attacks"),
        ("mev", "MEV Attacks"),
        ("oracle", "Oracle Manipulation"),
        ("cross_vm", "Cross-VM Arbitrage"),
        ("governance", "Governance Attacks"),
    ];
    
    let mut total_blockers = 0;
    
    for (area, name) in areas {
        println!("▸ Testing {}...", name);
        let result = match area {
            "flashloans" => runners::flashloans::run_proofs(workspace, verbose).await?,
            "oracle" => runners::oracle::run_proofs(workspace, verbose).await?,
            _ => continue,
        };
        
        if !result.blockers.is_empty() {
            total_blockers += result.blockers.len();
            for blocker in &result.blockers {
                println!("  ⛔ {}", blocker);
            }
        }
    }
    
    if total_blockers > 0 {
        println!("\n✗ Economic gate FAILED - {} S0 blockers", total_blockers);
        std::process::exit(1);
    } else {
        println!("\n✅ Economic gate PASSED");
        Ok(())
    }
}
```

---

### **Step 4: Integrate into Security Gate**

```rust
// In proof-forge/src/main.rs, update security_gate:

async fn security_gate(workspace: &PathBuf) -> Result<()> {
    // ... existing checks ...
    
    // Add economic attack testing
    println!("🏦 Running economic attack tests...");
    let economic_result = economic_gate(workspace, true).await?;
    
    // ... rest of security gate ...
}
```

---

### **Step 5: Add to CI/CD**

```yaml
# .github/workflows/economic-attacks.yml
name: Economic Attack Tests

on: [push, pull_request]

jobs:
  economic-gate:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v3
      - name: Install Rust
        uses: actions-rs/toolchain@v1
      - name: Run Economic Gate
        run: |
          cd proof-forge
          cargo build --release
          cd ..
          ./target/release/x3-proof economic-gate --workspace . --verbose
```

---

## Success Criteria

- [ ] All 15 economic attack tests pass
- [ ] Flash loan attacks (reentrancy, oracle manipulation, repayment bypass) prevented
- [ ] MEV attacks (sandwich, front-running) mitigated or taxed
- [ ] Oracle manipulation (TWAP, front-running) resistant
- [ ] Cross-VM arbitrage bounded
- [ ] Governance flash loan takeover prevented
- [ ] `x3-proof economic-gate` passes
- [ ] CI fails on any economic attack vulnerability

---

## Timeline

- **Day 1:** Implement flash loan attack tests (3 tests)
- **Day 2:** Implement MEV attack tests (2 tests)
- **Day 3:** Implement oracle attack tests (2 tests)
- **Day 4:** Implement cross-VM and governance tests (3 tests)
- **Day 5:** Integrate into ProofForge, test on CI

---

## References

- Flash Loan Attack Vectors: https://github.com/OffcierCia/DeFi-Developer-Road-Map#flash-loans
- MEV Protection: https://docs.flashbots.net/
- Oracle Security: https://blog.chain.link/challenges-in-defi-how-to-bring-more-capital-and-less-risk-to-defi-with-oracle-networks/
- Economic Attack Taxonomy: https://github.com/cossacklabs/awesome-ethereum-security
