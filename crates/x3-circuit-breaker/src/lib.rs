use std::collections::HashMap;

pub type ScopeId = [u8; 32];
pub type BlockNumber = u64;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CircuitBreakerScope {
    Asset(ScopeId),
    Route(ScopeId),
    Gateway(ScopeId),
    DexPool(ScopeId),
    Verifier(ScopeId),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CircuitBreakerStatus {
    Armed,
    Tripped,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CircuitBreakerRecord {
    pub scope: CircuitBreakerScope,
    pub status: CircuitBreakerStatus,
    pub reason: String,
    pub tripped_at_block: Option<BlockNumber>,
    pub reset_at_block: Option<BlockNumber>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CircuitBreakerError {
    GovernanceRequired,
}

#[derive(Debug, Default)]
pub struct CircuitBreakerEngine {
    records: HashMap<CircuitBreakerScope, CircuitBreakerRecord>,
    events: Vec<CircuitBreakerRecord>,
}

impl CircuitBreakerEngine {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn trip_circuit_breaker(
        &mut self,
        scope: CircuitBreakerScope,
        reason: impl Into<String>,
        now: BlockNumber,
    ) -> CircuitBreakerRecord {
        let record = CircuitBreakerRecord {
            scope,
            status: CircuitBreakerStatus::Tripped,
            reason: reason.into(),
            tripped_at_block: Some(now),
            reset_at_block: None,
        };
        self.records.insert(scope, record.clone());
        self.events.push(record.clone());
        record
    }

    pub fn reset_circuit_breaker(
        &mut self,
        scope: CircuitBreakerScope,
        privileged_origin: bool,
        now: BlockNumber,
    ) -> Result<CircuitBreakerRecord, CircuitBreakerError> {
        if !privileged_origin {
            return Err(CircuitBreakerError::GovernanceRequired);
        }
        let record = CircuitBreakerRecord {
            scope,
            status: CircuitBreakerStatus::Armed,
            reason: "reset".to_string(),
            tripped_at_block: None,
            reset_at_block: Some(now),
        };
        self.records.insert(scope, record.clone());
        self.events.push(record.clone());
        Ok(record)
    }

    pub fn is_circuit_breaker_tripped(&self, scope: CircuitBreakerScope) -> bool {
        self.records
            .get(&scope)
            .map(|record| record.status == CircuitBreakerStatus::Tripped)
            .unwrap_or(false)
    }

    pub fn get_circuit_breaker_status(&self, scope: CircuitBreakerScope) -> CircuitBreakerStatus {
        self.records
            .get(&scope)
            .map(|record| record.status)
            .unwrap_or(CircuitBreakerStatus::Armed)
    }

    pub fn enforce_deposit_allowed(&self, route_id: ScopeId) -> Result<(), CircuitBreakerScope> {
        let scope = CircuitBreakerScope::Route(route_id);
        if self.is_circuit_breaker_tripped(scope) {
            return Err(scope);
        }
        Ok(())
    }

    pub fn enforce_proof_acceptance_allowed(
        &self,
        verifier_id: ScopeId,
    ) -> Result<(), CircuitBreakerScope> {
        let scope = CircuitBreakerScope::Verifier(verifier_id);
        if self.is_circuit_breaker_tripped(scope) {
            return Err(scope);
        }
        Ok(())
    }

    pub fn enforce_swap_allowed(&self, pool_id: ScopeId) -> Result<(), CircuitBreakerScope> {
        let scope = CircuitBreakerScope::DexPool(pool_id);
        if self.is_circuit_breaker_tripped(scope) {
            return Err(scope);
        }
        Ok(())
    }

    pub fn events(&self) -> &[CircuitBreakerRecord] {
        &self.events
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn route_breaker_blocks_deposits() {
        let mut engine = CircuitBreakerEngine::new();
        engine.trip_circuit_breaker(CircuitBreakerScope::Route([1; 32]), "failure_spike", 10);

        assert!(engine.enforce_deposit_allowed([1; 32]).is_err());
    }

    #[test]
    fn verifier_breaker_blocks_proof_acceptance() {
        let mut engine = CircuitBreakerEngine::new();
        engine.trip_circuit_breaker(CircuitBreakerScope::Verifier([2; 32]), "quorum_failure", 10);

        assert!(engine.enforce_proof_acceptance_allowed([2; 32]).is_err());
    }

    #[test]
    fn pool_breaker_blocks_swaps() {
        let mut engine = CircuitBreakerEngine::new();
        engine.trip_circuit_breaker(
            CircuitBreakerScope::DexPool([3; 32]),
            "reserve_mismatch",
            10,
        );

        assert!(engine.enforce_swap_allowed([3; 32]).is_err());
    }

    #[test]
    fn reset_requires_governance() {
        let mut engine = CircuitBreakerEngine::new();
        engine.trip_circuit_breaker(CircuitBreakerScope::Route([1; 32]), "failure_spike", 10);

        assert!(matches!(
            engine.reset_circuit_breaker(CircuitBreakerScope::Route([1; 32]), false, 11),
            Err(CircuitBreakerError::GovernanceRequired)
        ));
        engine
            .reset_circuit_breaker(CircuitBreakerScope::Route([1; 32]), true, 12)
            .unwrap();
        assert_eq!(
            engine.get_circuit_breaker_status(CircuitBreakerScope::Route([1; 32])),
            CircuitBreakerStatus::Armed
        );
    }

    #[test]
    fn trip_event_is_recorded_without_supply_mutation() {
        let mut engine = CircuitBreakerEngine::new();
        engine.trip_circuit_breaker(
            CircuitBreakerScope::Gateway([4; 32]),
            "collateral_mismatch",
            7,
        );

        assert_eq!(engine.events().len(), 1);
        assert_eq!(engine.events()[0].status, CircuitBreakerStatus::Tripped);
    }
}
