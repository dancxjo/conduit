//! Core fidelity covers this numerical grid query only; complete rich-basis
//! provenance remains in the original prepared upstream owner and receipt.
use super::*;
use conduit_core::projection::*;
fn text(s: &str) -> ProjectionText<'_> {
    ProjectionText::new(s).unwrap()
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct PeriodPrecisionLoss {
    numerator: u64,
    denominator: u64,
    model_above_exact: bool,
}
pub(super) struct PeriodProjection<'a> {
    receipt: &'a PreparedRichPeriod,
    source: StructuredInfoValue,
    loss: PeriodPrecisionLoss,
    exact: bool,
}
impl<'a> PeriodProjection<'a> {
    pub(super) fn new(receipt: &'a PreparedRichPeriod) -> Self {
        let boolean = |n| {
            let StructuredInfoValueShape::Leaf(b) = field(&receipt.result, n).shape() else {
                panic!("Boolean")
            };
            InfoBool::decode(b).unwrap().get()
        };
        Self {
            receipt,
            source: StructuredInfoValue::from_canonical_bytes(&receipt.eligible).unwrap(),
            loss: PeriodPrecisionLoss {
                numerator: number(field(&receipt.result, "error_numerator")),
                denominator: number(field(&receipt.result, "error_denominator")),
                model_above_exact: boolean("model_above_exact"),
            },
            exact: boolean("exact"),
        }
    }
    pub(super) fn facts(&self) -> [ProjectionFact<'_, Self>; 3] {
        [
            ProjectionFact::Preserved {
                obligation: text("original-cycle-rate-and-declared-epoch-query"),
            },
            ProjectionFact::Preserved {
                obligation: text("caller-selected-quantization-policy-and-cadence"),
            },
            if self.exact {
                ProjectionFact::Transformed {
                    obligation: text("model-period-precision"),
                    law: text("ai/source-nearest-whole-sample-ties-up@1"),
                }
            } else {
                ProjectionFact::Lost {
                    obligation: text("model-period-precision"),
                    class: ProjectionLoss::Precision,
                    detail: &self.loss,
                    native_fact: Some(text("original-model-period-query")),
                }
            },
        ]
    }
    pub(super) fn native(&self) -> [ProjectionNativeFact<'_>; 1] {
        [ProjectionNativeFact {
            identity: text("original-model-period-query"),
            contract: text(self.receipt.profile),
            provider: text("ai/checked-source@1"),
            encoding: text("structured-native@1"),
            bytes: &self.receipt.eligible,
        }]
    }
    pub(super) fn input<'b>(
        &'b self,
        facts: &'b [ProjectionFact<'b, Self>],
        native: &'b [ProjectionNativeFact<'b>],
    ) -> ProjectionInput<'b, Self> {
        ProjectionInput {
            basis: ProjectionBasis {
                report: text("fargan/model-period-query@1"),
                source: text("retained-original-query"),
                target: Some(text("admitted-model-period")),
                projector: text("ai/source-nearest-period@1"),
                requested_route: text("fargan/16k-model-period@1"),
                boundary: None,
            },
            source: &self.source,
            target: Some(&self.receipt.period),
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
impl ProjectionDomain for PeriodProjection<'_> {
    type Source = StructuredInfoValue;
    type Target = StructuredInfoValue;
    type Detail = PeriodPrecisionLoss;
    fn source_contract(&self) -> ProjectionText<'_> {
        text(self.receipt.profile)
    }
    fn target_contract(&self) -> ProjectionText<'_> {
        text("speech/fargan-period-u16-32-255@1")
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
            || target != Some(&self.receipt.period)
            || mechanism != ProjectionMechanism::Completed
            || !scores.is_empty()
            || facts.len() != 3
            || native.len() != 1
        {
            return false;
        }
        let expected = self.native()[0];
        let actual = native[0];
        if actual.identity != expected.identity
            || actual.contract != expected.contract
            || actual.provider != expected.provider
            || actual.encoding != expected.encoding
            || actual.bytes != expected.bytes
        {
            return false;
        }
        for (f, n) in facts[..2].iter().zip([
            "original-cycle-rate-and-declared-epoch-query",
            "caller-selected-quantization-policy-and-cadence",
        ]) {
            if !matches!(f,ProjectionFact::Preserved{obligation}if obligation.as_str()==n) {
                return false;
            }
        }
        match &facts[2] {
            ProjectionFact::Transformed { obligation, law } => {
                self.exact
                    && obligation.as_str() == "model-period-precision"
                    && law.as_str() == "ai/source-nearest-whole-sample-ties-up@1"
            }
            ProjectionFact::Lost {
                obligation,
                class: ProjectionLoss::Precision,
                detail,
                native_fact: Some(id),
            } => {
                !self.exact
                    && obligation.as_str() == "model-period-precision"
                    && **detail == self.loss
                    && id.as_str() == "original-model-period-query"
            }
            _ => false,
        }
    }
}
pub(super) struct NearestWholeSamplePolicy;
impl ProjectionPolicy<PeriodProjection<'_>> for NearestWholeSamplePolicy {
    fn identity(&self) -> ProjectionText<'_> {
        text("ai/explicit-nearest-whole-sample-ties-up@1")
    }
    fn permits(
        &self,
        _: &StructuredInfoValue,
        _: &StructuredInfoValue,
        facts: &[ProjectionFact<'_, PeriodProjection<'_>>],
    ) -> bool {
        facts.len() == 3
            && matches!(&facts[2],ProjectionFact::Lost{class:ProjectionLoss::Precision,detail,..}if detail.numerator>0&&detail.denominator>0&&u128::from(detail.numerator)*2<=u128::from(detail.denominator))
    }
}
#[test]
fn rich_period_core_report_refuses_exact_consumer_and_undeclared_losses() {
    for (n, d, exact) in [(1, 200, true), (1, 300, false), (149, 32000, false)] {
        let receipt = prepare_fixture(n, d, 16000, 0, 0).unwrap();
        let domain = PeriodProjection::new(&receipt);
        let facts = domain.facts();
        let native = domain.native();
        let policy = NearestWholeSamplePolicy;
        let report =
            ProjectionReport::new(&domain, &policy, domain.input(&facts, &native)).unwrap();
        if exact {
            assert!(report.require_exact().is_ok())
        } else {
            assert_eq!(
                report.summary().disposition,
                ProjectionDisposition::Completed(ProjectionFidelity::PermittedLossy)
            );
            assert_eq!(
                report.require_exact(),
                Err(ProjectionRefusal::ConsumerRequiresExact)
            );
            assert!(
                report
                    .require_policy(text("ai/explicit-nearest-whole-sample-ties-up@1"))
                    .is_ok()
            );
        }
        assert!(
            ProjectionReport::new(&domain, &policy, domain.input(&facts[..2], &native)).is_err()
        );
        let wrong = [
            ProjectionFact::Preserved {
                obligation: text("original-cycle-rate-and-declared-epoch-query"),
            },
            ProjectionFact::Preserved {
                obligation: text("caller-selected-quantization-policy-and-cadence"),
            },
            ProjectionFact::Lost {
                obligation: text("model-period-precision"),
                class: ProjectionLoss::Approximation,
                detail: &domain.loss,
                native_fact: Some(text("original-model-period-query")),
            },
        ];
        assert!(ProjectionReport::new(&domain, &policy, domain.input(&wrong, &native)).is_err());
    }
}

pub(super) fn report_material(receipt: &PreparedRichPeriod) -> serde_json::Value {
    let domain = PeriodProjection::new(receipt);
    let policy = NearestWholeSamplePolicy;
    let facts = domain.facts();
    let native = domain.native();
    let report = ProjectionReport::new(&domain, &policy, domain.input(&facts, &native))
        .expect("Source-backed Core precision report");
    serde_json::json!({"source_contract":receipt.profile,"target_contract":"speech/fargan-period-u16-32-255@1","policy":"ai/explicit-nearest-whole-sample-ties-up@1","summary":format!("{:?}",report.summary()),"exact":domain.exact,"precision_error_numerator":domain.loss.numerator,"precision_error_denominator":domain.loss.denominator,"model_above_exact":domain.loss.model_above_exact,"original_native_fact":receipt.eligible,"scope":"numerical original cycle/rate/epoch query and selected policy only; upstream occurrence/provenance retained separately"})
}
