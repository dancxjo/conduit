#![cfg(feature = "semantic-bindings")]
//! A checkpoint-private symbol is a target artifact, not portable phone identity.
use conduit_core::projection::*;
use conduit_plot::rust_binding::BoundedSequence;
use conduit_speech::semantic::*;
fn t(s: &str) -> ProjectionText<'_> {
    ProjectionText::new(s).unwrap()
}
struct ModelProjection;
#[derive(PartialEq, Eq)]
enum PhoneLoss {
    AspirationNotExpressible,
}
struct PrivateSymbol {
    checkpoint: &'static str,
    symbol: u16,
}
impl ProjectionDomain for ModelProjection {
    type Source = SpeechPhone;
    type Target = PrivateSymbol;
    type Detail = PhoneLoss;
    fn source_contract(&self) -> ProjectionText<'_> {
        t("speech/phone@1")
    }
    fn target_contract(&self) -> ProjectionText<'_> {
        t("checkpoint/demo-vocab@1")
    }
    fn validate(
        &self,
        source: &SpeechPhone,
        target: Option<&PrivateSymbol>,
        facts: &[ProjectionFact<'_, Self>],
        native: &[ProjectionNativeFact<'_>],
        scores: &[ProjectionScore<'_>],
        _mechanism: ProjectionMechanism,
    ) -> bool {
        source.ipa() == "tʰ"
            && facts.len() == 1
            && native.is_empty()
            && scores.is_empty()
            && target.is_some_and(|target| {
                target.checkpoint == "checkpoint/demo/1" && target.symbol == 42
            })
            && matches!(facts[0],ProjectionFact::Lost {obligation,class:ProjectionLoss::Approximation,detail:PhoneLoss::AspirationNotExpressible,native_fact:None} if obligation.as_str()=="phone/aspiration")
    }
}
struct Approximation {
    allowed: bool,
}
impl ProjectionPolicy<ModelProjection> for Approximation {
    fn identity(&self) -> ProjectionText<'_> {
        if self.allowed {
            t("model/t-aspiration-budget-one@1")
        } else {
            t("model/strict@1")
        }
    }
    fn permits(
        &self,
        _: &SpeechPhone,
        _: &PrivateSymbol,
        facts: &[ProjectionFact<'_, ModelProjection>],
    ) -> bool {
        self.allowed
            && facts.len() == 1
            && matches!(
                facts[0],
                ProjectionFact::Lost {
                    class: ProjectionLoss::Approximation,
                    detail: PhoneLoss::AspirationNotExpressible,
                    ..
                }
            )
    }
}
#[test]
fn private_vocabulary_requires_exact_admission_of_aspiration_approximation() {
    let source = SpeechPhone::new(
        BoundedSequence::new(),
        SpeechFeatureBundle::new(BoundedSequence::new()).unwrap(),
        PhoneId::new("portable/aspirated-t".into()).unwrap(),
        "tʰ".into(),
        SpeechSegmentStatus::Allophonic,
    )
    .unwrap();
    let target = PrivateSymbol {
        checkpoint: "checkpoint/demo/1",
        symbol: 42,
    };
    let detail = PhoneLoss::AspirationNotExpressible;
    let facts = [ProjectionFact::Lost {
        obligation: t("phone/aspiration"),
        class: ProjectionLoss::Approximation,
        detail: &detail,
        native_fact: None,
    }];
    let domain = ModelProjection;
    let strict = Approximation { allowed: false };
    let policy = Approximation { allowed: true };
    let input = |facts| ProjectionInput {
        basis: ProjectionBasis {
            report: t("report/model/1"),
            source: t("phone/portable/aspirated-t"),
            target: Some(t("private-symbol/demo/42")),
            projector: t("phone-to-checkpoint@1"),
            requested_route: t("checkpoint/demo@1"),
            boundary: None,
        },
        source: &source,
        target: Some(&target),
        facts,
        native: &[],
        scores: &[],
        admitted: &[],
        attempts: &[],
        selected_attempt: None,
        mechanism: ProjectionMechanism::Completed,
        diagnostics: &[],
    };
    let partial = ProjectionReport::new(&domain, &strict, input(&facts)).unwrap();
    assert_eq!(
        partial.summary().disposition,
        ProjectionDisposition::Insufficient
    );
    assert!(partial.require_exact().is_err());
    let accepted = ProjectionReport::new(&domain, &policy, input(&facts)).unwrap();
    assert_eq!(
        accepted.summary().disposition,
        ProjectionDisposition::Completed(ProjectionFidelity::PermittedLossy)
    );
    assert_eq!(
        accepted
            .require_policy(t("model/t-aspiration-budget-one@1"))
            .unwrap()
            .symbol,
        42
    );
    assert!(accepted.require_policy(t("model/strict@1")).is_err());
    assert_eq!(accepted.source(), &source);
    assert_eq!(
        source.identity(),
        &PhoneId::new("portable/aspirated-t".into()).unwrap()
    );
    assert!(matches!(
        ProjectionReport::new(&domain, &policy, input(&[])),
        Err(ProjectionRefusal::Domain)
    ));
    let silent = [ProjectionFact::Preserved {
        obligation: t("phone/aspiration"),
    }];
    assert!(matches!(
        ProjectionReport::new(&domain, &policy, input(&silent)),
        Err(ProjectionRefusal::Domain)
    ));
}
