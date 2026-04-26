#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RiskTier {
    Low,
    Medium,
    High,
    Critical,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RouteRiskInput {
    pub value_usd: u64,
    pub recent_failures: u32,
    pub verifier_quorum_met: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RiskPolicy {
    pub high_value_usd_threshold: u64,
    pub critical_value_usd_threshold: u64,
    pub failure_threshold: u32,
    pub paused: bool,
}

impl Default for RiskPolicy {
    fn default() -> Self {
        Self {
            high_value_usd_threshold: 250_000,
            critical_value_usd_threshold: 1_000_000,
            failure_threshold: 5,
            paused: false,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RiskDecision {
    pub tier: RiskTier,
    pub allow_route: bool,
    pub reason: &'static str,
}

pub struct GatewayRiskEngine {
    policy: RiskPolicy,
}

impl GatewayRiskEngine {
    pub fn new(policy: RiskPolicy) -> Self {
        Self { policy }
    }

    pub fn policy(&self) -> RiskPolicy {
        self.policy
    }

    pub fn set_paused(&mut self, paused: bool) {
        self.policy.paused = paused;
    }

    pub fn evaluate(&self, input: RouteRiskInput) -> RiskDecision {
        if self.policy.paused {
            return RiskDecision {
                tier: RiskTier::Critical,
                allow_route: false,
                reason: "gateway_paused",
            };
        }

        if !input.verifier_quorum_met {
            return RiskDecision {
                tier: RiskTier::Critical,
                allow_route: false,
                reason: "verifier_quorum_missing",
            };
        }

        if input.recent_failures >= self.policy.failure_threshold {
            return RiskDecision {
                tier: RiskTier::High,
                allow_route: false,
                reason: "failure_rate_exceeded",
            };
        }

        if input.value_usd >= self.policy.critical_value_usd_threshold {
            return RiskDecision {
                tier: RiskTier::Critical,
                allow_route: false,
                reason: "critical_value_manual_review",
            };
        }

        if input.value_usd >= self.policy.high_value_usd_threshold {
            return RiskDecision {
                tier: RiskTier::High,
                allow_route: true,
                reason: "high_value_additional_monitoring",
            };
        }

        if input.value_usd >= self.policy.high_value_usd_threshold / 2 {
            return RiskDecision {
                tier: RiskTier::Medium,
                allow_route: true,
                reason: "medium_value",
            };
        }

        RiskDecision {
            tier: RiskTier::Low,
            allow_route: true,
            reason: "low_risk",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_when_gateway_is_paused() {
        let mut engine = GatewayRiskEngine::new(RiskPolicy::default());
        engine.set_paused(true);
        let decision = engine.evaluate(RouteRiskInput {
            value_usd: 10,
            recent_failures: 0,
            verifier_quorum_met: true,
        });

        assert!(!decision.allow_route);
        assert_eq!(decision.reason, "gateway_paused");
    }

    #[test]
    fn rejects_when_quorum_missing() {
        let engine = GatewayRiskEngine::new(RiskPolicy::default());
        let decision = engine.evaluate(RouteRiskInput {
            value_usd: 10,
            recent_failures: 0,
            verifier_quorum_met: false,
        });

        assert!(!decision.allow_route);
        assert_eq!(decision.reason, "verifier_quorum_missing");
    }

    #[test]
    fn rejects_when_recent_failures_exceed_threshold() {
        let engine = GatewayRiskEngine::new(RiskPolicy::default());
        let decision = engine.evaluate(RouteRiskInput {
            value_usd: 1_000,
            recent_failures: 5,
            verifier_quorum_met: true,
        });

        assert!(!decision.allow_route);
        assert_eq!(decision.reason, "failure_rate_exceeded");
    }

    #[test]
    fn allows_medium_risk_route() {
        let engine = GatewayRiskEngine::new(RiskPolicy::default());
        let decision = engine.evaluate(RouteRiskInput {
            value_usd: 130_000,
            recent_failures: 0,
            verifier_quorum_met: true,
        });

        assert!(decision.allow_route);
        assert_eq!(decision.tier, RiskTier::Medium);
    }

    #[test]
    fn blocks_critical_value_route_for_manual_review() {
        let engine = GatewayRiskEngine::new(RiskPolicy::default());
        let decision = engine.evaluate(RouteRiskInput {
            value_usd: 1_500_000,
            recent_failures: 0,
            verifier_quorum_met: true,
        });

        assert!(!decision.allow_route);
        assert_eq!(decision.reason, "critical_value_manual_review");
    }
}
