//! A class label cannot stand in for the exact admitted policy parameters.
use super::common::*;
use conduit_core::projection::*;
use core::cell::Cell;

struct ConfiguredBudget(Cell<u64>);
impl ProjectionPolicy<Bits> for ConfiguredBudget {
    fn identity(&self) -> ProjectionText<'_> {
        text("configured-budget@1")
    }
    fn configuration_digest(&self) -> Option<[u8; 32]> {
        Some(conduit_core::semantic_digest(
            "test/projection-budget@1",
            &self.0.get().to_le_bytes(),
        ))
    }
    fn permits(&self, source: &u64, target: &u64, facts: &[ProjectionFact<'_, Bits>]) -> bool {
        Budget(self.0.get() as usize).permits(source, target, facts)
    }
}
#[test]
fn exact_configuration_is_frozen_even_when_another_configuration_also_permits_the_target() {
    let domain = Bits;
    let source = 1;
    let facts = [ProjectionFact::Preserved {
        obligation: text("0"),
    }];
    let policy = ConfiguredBudget(Cell::new(1));
    let original = policy.configuration_digest().unwrap();
    let report =
        ProjectionReport::new(&domain, &policy, input(&source, Some(&source), &facts)).unwrap();
    assert_eq!(
        report.require_bound_policy(policy.identity(), original),
        Ok(&source)
    );
    policy.0.set(2);
    let later = policy.configuration_digest().unwrap();
    assert_ne!(original, later);
    assert!(policy.permits(&source, &source, &facts));
    assert_eq!(report.policy_configuration_digest(), Some(original));
    assert_eq!(
        report.require_bound_policy(policy.identity(), later),
        Err(ProjectionRefusal::PolicyMismatch)
    );
    assert_eq!(
        report.require_bound_policy(policy.identity(), original),
        Ok(&source)
    );
}
#[test]
fn configuration_bound_consumption_refuses_missing_evidence() {
    let domain = Bits;
    let policy = Budget(0);
    let source = 1;
    let facts = [ProjectionFact::Preserved {
        obligation: text("0"),
    }];
    let report =
        ProjectionReport::new(&domain, &policy, input(&source, Some(&source), &facts)).unwrap();
    assert_eq!(report.require_policy(policy.identity()), Ok(&source));
    assert_eq!(
        report.require_bound_policy(policy.identity(), [0; 32]),
        Err(ProjectionRefusal::PolicyConfigurationUnavailable)
    );
}
