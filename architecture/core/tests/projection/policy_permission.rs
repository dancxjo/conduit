//! Exact fidelity does not grant consumption under a denying policy.
use super::common::*;
use conduit_core::projection::*;
use core::cell::Cell;

struct SelectedPolicy {
    permitted: Cell<bool>,
    calls: Cell<usize>,
}
impl ProjectionPolicy<Bits> for SelectedPolicy {
    fn identity(&self) -> ProjectionText<'_> {
        text("selected@1")
    }
    fn permits(&self, _: &u64, _: &u64, _: &[ProjectionFact<'_, Bits>]) -> bool {
        self.calls.set(self.calls.get() + 1);
        self.permitted.get()
    }
}

#[test]
fn zero_loss_retains_exact_fidelity_but_denied_policy_cannot_consume() {
    let domain = Bits;
    let policy = SelectedPolicy {
        permitted: Cell::new(false),
        calls: Cell::new(0),
    };
    let source = 1;
    let facts = [ProjectionFact::Preserved {
        obligation: text("0"),
    }];
    let report =
        ProjectionReport::new(&domain, &policy, input(&source, Some(&source), &facts)).unwrap();
    assert_eq!(report.require_exact(), Ok(&source));
    assert_eq!(
        report.require_policy(text("selected@1")),
        Err(ProjectionRefusal::PolicyMismatch)
    );
    assert_eq!(
        report.require_policy(text("other@1")),
        Err(ProjectionRefusal::PolicyMismatch)
    );
    assert_eq!(policy.calls.get(), 1);
    // Changing policy state after admission cannot grant a retained report authority.
    policy.permitted.set(true);
    assert_eq!(
        report.require_policy(text("selected@1")),
        Err(ProjectionRefusal::PolicyMismatch)
    );
    assert_eq!(policy.calls.get(), 1);
}

#[test]
fn admitted_permission_is_frozen_for_exact_and_lossy_reports() {
    let domain = Bits;
    let source = 1;
    let exact = [ProjectionFact::Preserved {
        obligation: text("0"),
    }];
    let detail = 0;
    let lost = [ProjectionFact::Lost {
        obligation: text("0"),
        class: ProjectionLoss::Approximation,
        detail: &detail,
        native_fact: None,
    }];
    for (target, facts, fidelity) in [
        (1, &exact[..], ProjectionFidelity::Exact),
        (0, &lost[..], ProjectionFidelity::PermittedLossy),
    ] {
        let policy = SelectedPolicy {
            permitted: Cell::new(true),
            calls: Cell::new(0),
        };
        let report =
            ProjectionReport::new(&domain, &policy, input(&source, Some(&target), facts)).unwrap();
        policy.permitted.set(false);
        assert_eq!(
            report.summary().disposition,
            ProjectionDisposition::Completed(fidelity)
        );
        assert_eq!(report.require_policy(text("selected@1")), Ok(&target));
        assert_eq!(policy.calls.get(), 1);
    }
}
