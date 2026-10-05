use super::common::*;
use conduit_core::projection::*;

#[test]
fn fidelity_and_strict_consumption_derive_from_complete_inventory() {
    let domain = Bits;
    let strict = Budget(0);
    let permits = Budget(1);
    let source = 3;
    let target = 1;
    let detail = 1;
    let facts = [
        ProjectionFact::Preserved {
            obligation: text("0"),
        },
        ProjectionFact::Lost {
            obligation: text("1"),
            class: ProjectionLoss::Approximation,
            detail: &detail,
            native_fact: None,
        },
    ];
    let partial =
        ProjectionReport::new(&domain, &strict, input(&source, Some(&target), &facts)).unwrap();
    assert_eq!(
        partial.summary().disposition,
        ProjectionDisposition::Insufficient
    );
    assert_eq!(
        partial.require_exact(),
        Err(ProjectionRefusal::ConsumerRequiresExact)
    );
    assert_eq!(partial.inspect_target(), Some(&target));
    let lossy =
        ProjectionReport::new(&domain, &permits, input(&source, Some(&target), &facts)).unwrap();
    assert_eq!(
        lossy.summary().disposition,
        ProjectionDisposition::Completed(ProjectionFidelity::PermittedLossy)
    );
    assert_eq!(
        lossy.require_exact(),
        Err(ProjectionRefusal::ConsumerRequiresExact)
    );
    assert_eq!(lossy.require_policy(text("loss-budget@1")), Ok(&target));
    assert_eq!(
        lossy.require_policy(text("strict@1")),
        Err(ProjectionRefusal::PolicyMismatch)
    );
    assert!(matches!(
        ProjectionReport::new(&domain, &strict, input(&source, Some(&target), &facts[..1])),
        Err(ProjectionRefusal::Domain)
    ));
    let wrong = 3;
    assert!(matches!(
        ProjectionReport::new(&domain, &permits, input(&source, Some(&wrong), &facts)),
        Err(ProjectionRefusal::Domain)
    ));
}

#[test]
fn transformation_is_exact_but_undeclared_law_refuses() {
    let domain = Bits;
    let policy = Budget(0);
    let source = 1;
    let target = 1;
    let facts = [ProjectionFact::Transformed {
        obligation: text("0"),
        law: text("identity-layout@1"),
    }];
    let report =
        ProjectionReport::new(&domain, &policy, input(&source, Some(&target), &facts)).unwrap();
    assert_eq!(report.summary().transformed, 1);
    assert_eq!(report.require_exact(), Ok(&target));
    let wrong = [ProjectionFact::Transformed {
        obligation: text("0"),
        law: text("wishful@1"),
    }];
    assert!(matches!(
        ProjectionReport::new(&domain, &policy, input(&source, Some(&target), &wrong)),
        Err(ProjectionRefusal::Domain)
    ));
}

#[test]
fn every_loss_class_is_explicit_and_budget_is_aggregate() {
    let classes = [
        ProjectionLoss::Unrepresentable,
        ProjectionLoss::Unrecognized,
        ProjectionLoss::Precision,
        ProjectionLoss::Range,
        ProjectionLoss::Cardinality,
        ProjectionLoss::Ordering,
        ProjectionLoss::TemporalFinality,
        ProjectionLoss::Identity,
        ProjectionLoss::Provenance,
        ProjectionLoss::UnsupportedVariant,
        ProjectionLoss::Truncation,
        ProjectionLoss::Approximation,
    ];
    let domain = Bits;
    let strict = Budget(0);
    let policy = Budget(1);
    let source = 1;
    let target = 0;
    let detail = 0;
    let native = [ProjectionNativeFact {
        identity: text("unknown"),
        contract: text("external@1"),
        provider: text("provider@1"),
        encoding: text("opaque@1"),
        bytes: b"XYZ",
    }];
    for class in classes {
        let facts = [ProjectionFact::Lost {
            obligation: text("0"),
            class,
            detail: &detail,
            native_fact: Some(text("unknown")),
        }];
        let mut draft = input(&source, Some(&target), &facts);
        draft.native = &native;
        let partial = ProjectionReport::new(&domain, &strict, draft).unwrap();
        assert_eq!(partial.summary().losses[class as usize], 1);
        assert_eq!(
            partial.summary().truncated(),
            class == ProjectionLoss::Truncation
        );
        let mut draft = input(&source, Some(&target), &facts);
        draft.native = &native;
        assert_eq!(
            ProjectionReport::new(&domain, &policy, draft)
                .unwrap()
                .summary()
                .disposition,
            ProjectionDisposition::Completed(ProjectionFidelity::PermittedLossy)
        );
    }
    let source = 3;
    let second = 1;
    let facts = [
        ProjectionFact::Lost {
            obligation: text("0"),
            class: ProjectionLoss::Truncation,
            detail: &detail,
            native_fact: None,
        },
        ProjectionFact::Lost {
            obligation: text("1"),
            class: ProjectionLoss::Approximation,
            detail: &second,
            native_fact: None,
        },
    ];
    assert_eq!(
        ProjectionReport::new(&domain, &policy, input(&source, Some(&target), &facts))
            .unwrap()
            .summary()
            .disposition,
        ProjectionDisposition::Insufficient
    );
}
