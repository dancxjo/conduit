use super::common::*;
use conduit_core::projection::*;
use conduit_core::{
    ArtifactId, BootId, HostId, ImplementationId, PlanId, TerminalCategory, TerminalInfo,
};

#[test]
fn terminal_outcomes_are_distinct_and_categories_cannot_be_relabelled() {
    let domain = Bits;
    let policy = Budget(0);
    let source = 0;
    let refusal = TerminalInfo::new(TerminalCategory::RefusalOrAdmission, [1; 32]);
    let failure = TerminalInfo::new(TerminalCategory::ExecutionFault, [2; 32]);
    let cancel = TerminalInfo::new(TerminalCategory::Cancelled, [3; 32]);
    for (mechanism, expected) in [
        (
            ProjectionMechanism::Refused(refusal),
            ProjectionDisposition::Refused(refusal),
        ),
        (
            ProjectionMechanism::Failed(failure),
            ProjectionDisposition::Failed(failure),
        ),
        (
            ProjectionMechanism::Cancelled(cancel),
            ProjectionDisposition::Cancelled(cancel),
        ),
        (
            ProjectionMechanism::Completed,
            ProjectionDisposition::Insufficient,
        ),
    ] {
        let mut draft = input(&source, None, &[]);
        draft.mechanism = mechanism;
        assert_eq!(
            ProjectionReport::new(&domain, &policy, draft)
                .unwrap()
                .summary()
                .disposition,
            expected
        );
    }
    let mut draft = input(&source, None, &[]);
    draft.mechanism = ProjectionMechanism::Failed(cancel);
    assert!(matches!(
        ProjectionReport::new(&domain, &policy, draft),
        Err(ProjectionRefusal::TerminalCategory)
    ));
}

#[test]
fn fallback_records_only_admitted_realizations_and_exact_terminal_chain() {
    let domain = Bits;
    let policy = Budget(0);
    let source = 1;
    let facts = [ProjectionFact::Preserved {
        obligation: text("0"),
    }];
    let plan = PlanId::from("plan/1");
    let host = HostId::from("host/1");
    let boot = BootId::from("boot/1");
    let back = ImplementationId::from("back/1");
    let other = ImplementationId::from("back/2");
    let artifact = ArtifactId::from("artifact/1");
    let first = ProjectionRealization {
        plan: &plan,
        host: &host,
        boot: &boot,
        back: &back,
        artifact: &artifact,
    };
    let second = ProjectionRealization {
        back: &other,
        ..first
    };
    let admitted = [first, second];
    let failure = ProjectionMechanism::Failed(TerminalInfo::new(
        TerminalCategory::ProviderOrResourceLoss,
        [7; 32],
    ));
    let attempts = [
        ProjectionAttempt {
            realization: first,
            outcome: failure,
            fallback_reason: None,
        },
        ProjectionAttempt {
            realization: second,
            outcome: ProjectionMechanism::Completed,
            fallback_reason: Some(text("admitted-alternative/resource-loss@1")),
        },
    ];
    let mut draft = input(&source, Some(&source), &facts);
    draft.admitted = &admitted;
    draft.attempts = &attempts;
    draft.selected_attempt = Some(1);
    let report = ProjectionReport::new(&domain, &policy, draft).unwrap();
    assert_eq!(report.selected_attempt().unwrap().realization, second);
    assert_eq!(report.attempts()[0].outcome, failure);
    let mut draft = input(&source, Some(&source), &facts);
    draft.admitted = &admitted[..1];
    draft.attempts = &attempts;
    draft.selected_attempt = Some(1);
    assert!(matches!(
        ProjectionReport::new(&domain, &policy, draft),
        Err(ProjectionRefusal::Realization)
    ));
    let missing = [
        attempts[0],
        ProjectionAttempt {
            fallback_reason: None,
            ..attempts[1]
        },
    ];
    let mut draft = input(&source, Some(&source), &facts);
    draft.admitted = &admitted;
    draft.attempts = &missing;
    draft.selected_attempt = Some(1);
    assert!(matches!(
        ProjectionReport::new(&domain, &policy, draft),
        Err(ProjectionRefusal::FallbackReason)
    ));
    let mut draft = input(&source, Some(&source), &facts);
    draft.admitted = &admitted;
    draft.attempts = &attempts;
    draft.selected_attempt = Some(0);
    assert!(matches!(
        ProjectionReport::new(&domain, &policy, draft),
        Err(ProjectionRefusal::AttemptOutcome)
    ));
}

#[test]
fn finite_bounds_unknown_references_and_duplicate_obligations_refuse() {
    assert!(ProjectionText::new("").is_err());
    assert!(ProjectionText::new(&"x".repeat(193)).is_err());
    let domain = Bits;
    let policy = Budget(0);
    let source = 1;
    let detail = 0;
    let duplicates = [
        ProjectionFact::Preserved {
            obligation: text("0"),
        },
        ProjectionFact::Preserved {
            obligation: text("0"),
        },
    ];
    assert!(matches!(
        ProjectionReport::new(&domain, &policy, input(&source, Some(&source), &duplicates)),
        Err(ProjectionRefusal::DuplicateIdentity)
    ));
    let excessive: Vec<_> = (0..65)
        .map(|_| ProjectionFact::Preserved {
            obligation: text("0"),
        })
        .collect();
    assert!(matches!(
        ProjectionReport::new(&domain, &policy, input(&source, Some(&source), &excessive)),
        Err(ProjectionRefusal::FactBound)
    ));
    let unknown = [ProjectionFact::Lost {
        obligation: text("0"),
        class: ProjectionLoss::Unrecognized,
        detail: &detail,
        native_fact: None,
    }];
    assert!(matches!(
        ProjectionReport::new(&domain, &policy, input(&source, Some(&source), &unknown)),
        Err(ProjectionRefusal::UnknownNativeFact)
    ));
    let native = ProjectionNativeFact {
        identity: text("native"),
        contract: text("contract"),
        provider: text("provider"),
        encoding: text("opaque"),
        bytes: b"x",
    };
    let oversized = vec![native; 17];
    let facts = &duplicates[..1];
    let mut draft = input(&source, Some(&source), facts);
    draft.native = &oversized;
    assert!(matches!(
        ProjectionReport::new(&domain, &policy, draft),
        Err(ProjectionRefusal::NativeBound)
    ));
    let bytes = vec![0; 4097];
    let excessive_bytes = [ProjectionNativeFact {
        bytes: &bytes,
        ..native
    }];
    let mut draft = input(&source, Some(&source), facts);
    draft.native = &excessive_bytes;
    assert!(matches!(
        ProjectionReport::new(&domain, &policy, draft),
        Err(ProjectionRefusal::NativeBound)
    ));
    let scores = vec![
        ProjectionScore::Native {
            fact: text("native"),
            score: score("cost", "provider", 1)
        };
        9
    ];
    let mut draft = input(&source, Some(&source), facts);
    draft.scores = &scores;
    assert!(matches!(
        ProjectionReport::new(&domain, &policy, draft),
        Err(ProjectionRefusal::ScoreBound)
    ));
    let diagnostics = vec![text("bounded diagnostic"); 9];
    let mut draft = input(&source, Some(&source), facts);
    draft.diagnostics = &diagnostics;
    assert!(matches!(
        ProjectionReport::new(&domain, &policy, draft),
        Err(ProjectionRefusal::DiagnosticBound)
    ));
    let mut draft = input(&source, Some(&source), facts);
    draft.basis.target = None;
    assert!(matches!(
        ProjectionReport::new(&domain, &policy, draft),
        Err(ProjectionRefusal::TargetIdentity)
    ));
}

#[test]
fn unattempted_refusal_does_not_claim_preservation_or_need_a_target() {
    let domain = Bits;
    let policy = Budget(0);
    let source = 1;
    let refusal = ProjectionMechanism::Refused(TerminalInfo::new(
        TerminalCategory::RefusalOrAdmission,
        [4; 32],
    ));
    let mut draft = input(&source, None, &[]);
    draft.mechanism = refusal;
    assert!(ProjectionReport::new(&domain, &policy, draft).is_ok());
    let facts = [ProjectionFact::Preserved {
        obligation: text("0"),
    }];
    let mut draft = input(&source, None, &facts);
    draft.mechanism = refusal;
    assert!(matches!(
        ProjectionReport::new(&domain, &policy, draft),
        Err(ProjectionRefusal::AttemptOutcome)
    ));
}
#[test]
fn attempt_pressure_and_unknown_native_identity_refuse_before_domain_admission() {
    let domain = Bits;
    let policy = Budget(0);
    let source = 1;
    let detail = 0;
    let facts = [ProjectionFact::Lost {
        obligation: text("0"),
        class: ProjectionLoss::Unrecognized,
        detail: &detail,
        native_fact: Some(text("absent")),
    }];
    assert!(matches!(
        ProjectionReport::new(&domain, &policy, input(&source, Some(&0), &facts)),
        Err(ProjectionRefusal::UnknownNativeFact)
    ));
    let plan = PlanId::from("plan/1");
    let host = HostId::from("host/1");
    let boot = BootId::from("boot/1");
    let back = ImplementationId::from("back/1");
    let artifact = ArtifactId::from("artifact/1");
    let realization = ProjectionRealization {
        plan: &plan,
        host: &host,
        boot: &boot,
        back: &back,
        artifact: &artifact,
    };
    let excessive = vec![realization; 9];
    let mut draft = input(&0, Some(&0), &[]);
    draft.admitted = &excessive;
    assert!(matches!(
        ProjectionReport::new(&domain, &policy, draft),
        Err(ProjectionRefusal::AttemptBound)
    ));
    let attempts = vec![
        ProjectionAttempt {
            realization,
            outcome: ProjectionMechanism::Completed,
            fallback_reason: None
        };
        9
    ];
    let mut draft = input(&0, Some(&0), &[]);
    draft.attempts = &attempts;
    assert!(matches!(
        ProjectionReport::new(&domain, &policy, draft),
        Err(ProjectionRefusal::AttemptBound)
    ));
}

#[test]
fn maximum_selected_attempt_index_is_typed_refusal_without_overflow() {
    let domain = Bits;
    let policy = Budget(0);
    let source = 0;
    let mut draft = input(&source, Some(&source), &[]);
    draft.selected_attempt = Some(usize::MAX);
    assert!(matches!(
        ProjectionReport::new(&domain, &policy, draft),
        Err(ProjectionRefusal::AttemptOutcome)
    ));
}
