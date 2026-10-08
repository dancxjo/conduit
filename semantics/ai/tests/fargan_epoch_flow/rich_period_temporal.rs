//! Temporal onset/hold fidelity; independent of nearest-sample numeric precision.
use super::*;
use conduit_core::projection::*;
fn text(s: &str) -> ProjectionText<'_> {
    ProjectionText::new(s).unwrap()
}
#[derive(Debug, PartialEq, Eq)]
struct HoldApproximation {
    frames: u64,
    rate: u64,
}
struct HoldProjection<'a> {
    aggregate: &'a Aggregate,
    receipt: &'a Receipt,
    source: StructuredInfoValue,
    target: StructuredInfoValue,
    detail: HoldApproximation,
}
impl<'a> HoldProjection<'a> {
    fn new(a: &'a Aggregate, ordinal: usize) -> Result<Self, String> {
        let receipt = a.receipts.get(ordinal).ok_or("ordinal")?;
        if !Rc::ptr_eq(&a.basis, &receipt.basis) {
            return Err("foreign trajectory owner".into());
        }
        let d = &a.projector.document;
        let source = interface::admit_retained_session_native(
            d,
            "FarganRichHoldPosition",
            &receipt.position,
        )?;
        let span =
            interface::admit_retained_session_native(d, "FarganRichHoldSpan", &receipt.span)?;
        let q8 = interface::admit_retained_session_native(
            d,
            "SpeechCycleQ8AtRate",
            receipt.numerical.original_q8(),
        )?;
        let target_bytes = record(
            ty(d, "FarganRichHeldPeriodTarget"),
            &[
                ("span", span),
                ("original_q8", q8),
                (
                    "model_period",
                    field(receipt.numerical.result(), "period").clone(),
                ),
                ("policy", field(&source, "policy").clone()),
            ],
        )
        .canonical_bytes()
        .map_err(|e| format!("{e:?}"))?;
        let target = interface::admit_retained_session_native(
            d,
            "FarganRichHeldPeriodTarget",
            &target_bytes,
        )?;
        interface::admit_retained_session_native(
            d,
            "SpeechLinearPitchTrajectory",
            &a.basis.trajectory,
        )?;
        // Verify the exact retained fixed Source execution, not a Native assertion.
        let p = PortableExpressionProgram::from_canonical_hex(&a.hold_program)
            .map_err(|e| format!("{e:?}"))?;
        if p.evaluate(&receipt.position)
            .map_err(|e| format!("{e:?}"))?
            != receipt.span
        {
            return Err("foreign Source hold result".into());
        }
        Ok(Self {
            aggregate: a,
            receipt,
            source,
            target,
            detail: HoldApproximation {
                frames: 160,
                rate: 16000,
            },
        })
    }
    fn facts(&self) -> [ProjectionFact<'_, Self>; 2] {
        [
            ProjectionFact::Preserved {
                obligation: text("original-full-trajectory-and-source-hold-query"),
            },
            ProjectionFact::Lost {
                obligation: text("within-epoch-trajectory-variation"),
                class: ProjectionLoss::Approximation,
                detail: &self.detail,
                native_fact: Some(text("original-linear-cycle-trajectory")),
            },
        ]
    }
    fn native(&self) -> [ProjectionNativeFact<'_>; 2] {
        [
            ProjectionNativeFact {
                identity: text("original-linear-cycle-trajectory"),
                contract: text("speech/linear-pitch-trajectory@1"),
                provider: text("speech/checked-source@1"),
                encoding: text("structured-native@1"),
                bytes: &self.aggregate.basis.trajectory,
            },
            ProjectionNativeFact {
                identity: text("source-hold-query"),
                contract: text("ai/fargan-rich-hold-position@1"),
                provider: text("ai/checked-source@1"),
                encoding: text("structured-native@1"),
                bytes: &self.receipt.position,
            },
        ]
    }
    fn input<'b>(
        &'b self,
        facts: &'b [ProjectionFact<'b, Self>],
        native: &'b [ProjectionNativeFact<'b>],
    ) -> ProjectionInput<'b, Self> {
        ProjectionInput {
            basis: ProjectionBasis {
                report: text("fargan/temporal-onset-hold@1"),
                source: text("original-linear-cycle-trajectory-query"),
                target: Some(text("source-half-open-held-model-period")),
                projector: text("ai/source-onset-hold-10ms@1"),
                requested_route: text("fargan/16k-160-frame-cadence@1"),
                boundary: None,
            },
            source: &self.source,
            target: Some(&self.target),
            facts,
            native,
            scores: &[],
            admitted: &[],
            attempts: &[],
            selected_attempt: None,
            mechanism: ProjectionMechanism::Completed,
            diagnostics: &[],
        }
    }
}
impl ProjectionDomain for HoldProjection<'_> {
    type Source = StructuredInfoValue;
    type Target = StructuredInfoValue;
    type Detail = HoldApproximation;
    fn source_contract(&self) -> ProjectionText<'_> {
        text("ai/source-hold-position-with-retained-trajectory@1")
    }
    fn target_contract(&self) -> ProjectionText<'_> {
        text("ai/source-half-open-held-period@1")
    }
    fn validate(
        &self,
        source: &Self::Source,
        target: Option<&Self::Target>,
        facts: &[ProjectionFact<'_, Self>],
        native: &[ProjectionNativeFact<'_>],
        scores: &[ProjectionScore<'_>],
        mechanism: ProjectionMechanism,
    ) -> bool {
        if source != &self.source
            || target != Some(&self.target)
            || facts.len() != 2
            || native.len() != 2
            || !scores.is_empty()
            || mechanism != ProjectionMechanism::Completed
        {
            return false;
        }
        for (actual, expected) in native.iter().zip(self.native()) {
            if actual.identity != expected.identity
                || actual.contract != expected.contract
                || actual.provider != expected.provider
                || actual.encoding != expected.encoding
                || actual.bytes != expected.bytes
            {
                return false;
            }
        }
        matches!(&facts[0],ProjectionFact::Preserved{obligation}if obligation.as_str()=="original-full-trajectory-and-source-hold-query")
            && matches!(&facts[1],ProjectionFact::Lost{obligation,class:ProjectionLoss::Approximation,detail,native_fact:Some(id)}if obligation.as_str()=="within-epoch-trajectory-variation"&&**detail==self.detail&&id.as_str()=="original-linear-cycle-trajectory")
    }
}
struct DeclaredHoldPolicy<'a>(&'a str);
impl ProjectionPolicy<HoldProjection<'_>> for DeclaredHoldPolicy<'_> {
    fn identity(&self) -> ProjectionText<'_> {
        text(self.0)
    }
    fn permits(
        &self,
        _: &StructuredInfoValue,
        _: &StructuredInfoValue,
        facts: &[ProjectionFact<'_, HoldProjection<'_>>],
    ) -> bool {
        self.0 == "ai/explicit-onset-hold-10ms@1"
            && facts.len() == 2
            && matches!(&facts[1],ProjectionFact::Lost{class:ProjectionLoss::Approximation,detail,..}if detail.frames==160&&detail.rate==16000)
    }
}
pub(super) fn verify_report(a: &Aggregate, ordinal: usize) -> Result<(), String> {
    let domain = HoldProjection::new(a, ordinal)?;
    let facts = domain.facts();
    let native = domain.native();
    let policy = DeclaredHoldPolicy("ai/explicit-onset-hold-10ms@1");
    let report = ProjectionReport::new(&domain, &policy, domain.input(&facts, &native))
        .map_err(|e| format!("{e:?}"))?;
    assert_eq!(
        report.summary().disposition,
        ProjectionDisposition::Completed(ProjectionFidelity::PermittedLossy)
    );
    assert_eq!(
        report.require_exact(),
        Err(ProjectionRefusal::ConsumerRequiresExact)
    );
    let foreign_policy = DeclaredHoldPolicy("foreign/undeclared");
    let foreign =
        ProjectionReport::new(&domain, &foreign_policy, domain.input(&facts, &native)).unwrap();
    assert_eq!(
        foreign.summary().disposition,
        ProjectionDisposition::Insufficient
    );
    assert!(
        foreign
            .require_policy(text("ai/explicit-onset-hold-10ms@1"))
            .is_err()
    );
    assert!(report.require_policy(text("foreign/undeclared")).is_err());
    report
        .require_policy(text("ai/explicit-onset-hold-10ms@1"))
        .unwrap();
    assert!(ProjectionReport::new(&domain, &policy, domain.input(&facts[..1], &native)).is_err());
    let wrong = [
        ProjectionFact::Preserved {
            obligation: text("original-full-trajectory-and-source-hold-query"),
        },
        ProjectionFact::Lost {
            obligation: text("within-epoch-trajectory-variation"),
            class: ProjectionLoss::Precision,
            detail: &domain.detail,
            native_fact: Some(text("original-linear-cycle-trajectory")),
        },
    ];
    assert!(ProjectionReport::new(&domain, &policy, domain.input(&wrong, &native)).is_err());
    Ok(())
}

pub(super) fn material(a: &Aggregate, ordinal: usize) -> Result<serde_json::Value, String> {
    let domain = HoldProjection::new(a, ordinal)?;
    let facts = domain.facts();
    let native = domain.native();
    let policy = DeclaredHoldPolicy("ai/explicit-onset-hold-10ms@1");
    let report = ProjectionReport::new(&domain, &policy, domain.input(&facts, &native))
        .map_err(|e| format!("{e:?}"))?;
    report
        .require_policy(text("ai/explicit-onset-hold-10ms@1"))
        .map_err(|e| format!("{e:?}"))?;
    Ok(
        serde_json::json!({"policy":"ai/explicit-onset-hold-10ms@1","class":"Approximation","summary":format!("{:?}",report.summary()),"original_trajectory":a.basis.trajectory.as_ref(),"original_revision_effect_material":a.basis.revision_effect.as_ref(),"source_position":domain.receipt.position,"source_program":a.hold_program,"raw_admitted_span":domain.receipt.span,"admitted_held_target":domain.target.canonical_bytes().unwrap(),"numeric_precision_report":projection::report_material(&domain.receipt.numerical),"scope":"allocating component fixture; upstream actual committed owner/frame binding required separately; no clock authority or acoustic-error bound"}),
    )
}
