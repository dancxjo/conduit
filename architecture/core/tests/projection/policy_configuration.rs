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

struct ExplicitPermission {
    allowed: Cell<bool>,
    calls: Cell<usize>,
}
impl ProjectionPolicy<Bits> for ExplicitPermission {
    fn identity(&self) -> ProjectionText<'_> {
        text("explicit-permission@1")
    }
    fn configuration_digest(&self) -> Option<[u8; 32]> {
        Some(conduit_core::semantic_digest(
            "test/explicit-permission@1",
            &[u8::from(self.allowed.get())],
        ))
    }
    fn permits(&self, _: &u64, _: &u64, _: &[ProjectionFact<'_, Bits>]) -> bool {
        self.calls.set(self.calls.get() + 1);
        self.allowed.get()
    }
}
#[test]
fn zero_loss_still_requires_selected_policy_permission_and_freezes_that_decision() {
    let source = 1;
    let facts = [ProjectionFact::Preserved {
        obligation: text("0"),
    }];
    for allowed in [false, true] {
        let policy = ExplicitPermission {
            allowed: Cell::new(allowed),
            calls: Cell::new(0),
        };
        let digest = policy.configuration_digest().unwrap();
        let report =
            ProjectionReport::new(&Bits, &policy, input(&source, Some(&source), &facts)).unwrap();
        assert_eq!(
            policy.calls.get(),
            1,
            "exact facts cannot bypass the selected policy"
        );
        assert_eq!(report.inspect_target(), Some(&source));
        assert_eq!(report.summary().preserved, 1);
        assert_eq!(
            report
                .require_bound_policy(policy.identity(), digest)
                .is_ok(),
            allowed
        );
        assert_eq!(
            report.require_exact(),
            Ok(&source),
            "fidelity stays exact even if this policy denies its use"
        );
        policy.allowed.set(!allowed);
        assert_eq!(
            report
                .require_bound_policy(policy.identity(), digest)
                .is_ok(),
            allowed
        );
        assert_eq!(
            policy.calls.get(),
            1,
            "consumption uses the frozen admission decision"
        );
    }
}
