//! Acoustic-to-polynomial coefficient fidelity; no universal error enclosure.
use crate::resonator_projection_preparation::{field, integer};
use crate::ResonatorNumericReceipt;
use alloc::string::String;
use conduit_core::{projection::*, StructuredInfoValue};
#[derive(Debug, PartialEq, Eq)]
pub struct ResonatorNumericTarget {
    original: StructuredInfoValue,
    b_q14: i128,
    c_q14: i128,
    decay_q30: i128,
    cosine_q30: i128,
}
impl ResonatorNumericTarget {
    pub fn original(&self) -> &StructuredInfoValue {
        &self.original
    }
    pub fn b_q14(&self) -> i128 {
        self.b_q14
    }
    pub fn c_q14(&self) -> i128 {
        self.c_q14
    }
    pub fn decay_q30(&self) -> i128 {
        self.decay_q30
    }
    pub fn cosine_q30(&self) -> i128 {
        self.cosine_q30
    }
}
/// Names corpus conformance only, never a certified per-input bound.
#[derive(Debug, PartialEq, Eq)]
pub struct ResonatorPolynomialLoss;
impl ResonatorPolynomialLoss {
    pub fn profile(&self) -> &'static str {
        "polynomial_q30_to_q14_v1"
    }
    pub fn corpus_center_tolerance(&self) -> &'static str {
        "sample_rate_hz/8000 Hz; tested corpus only"
    }
    pub fn corpus_bandwidth_tolerance(&self) -> &'static str {
        "sample_rate_hz/20000 Hz; tested corpus only"
    }
    pub fn corpus_response_tolerance(&self) -> &'static str {
        "0.002+0.005*analytic magnitude; retained impulse fixture only"
    }
    pub fn rounding(&self) -> &'static str {
        "Q30 intermediate truncation toward zero; Q14 nearest ties away from zero"
    }
    pub fn certified_per_request_bound(&self) -> Option<&'static str> {
        None
    }
}
pub struct ResonatorNumericProjection<'a> {
    receipt: &'a ResonatorNumericReceipt,
    target: ResonatorNumericTarget,
    loss: ResonatorPolynomialLoss,
}
fn text(s: &str) -> ProjectionText<'_> {
    ProjectionText::new(s).expect("bounded contract")
}
impl<'a> ResonatorNumericProjection<'a> {
    pub fn new(receipt: &'a ResonatorNumericReceipt) -> Result<Self, String> {
        let candidate = receipt.candidate();
        if field(candidate, "request")? != receipt.original() {
            return Err(String::from("resonator original mismatch"));
        }
        Ok(Self {
            receipt,
            target: ResonatorNumericTarget {
                original: receipt.original().clone(),
                b_q14: integer(candidate, "b_q14")?,
                c_q14: integer(candidate, "c_q14")?,
                decay_q30: integer(candidate, "decay_q30")?,
                cosine_q30: integer(candidate, "cosine_q30")?,
            },
            loss: ResonatorPolynomialLoss,
        })
    }
    pub fn source(&self) -> &ResonatorNumericReceipt {
        self.receipt
    }
    pub fn target(&self) -> &ResonatorNumericTarget {
        &self.target
    }
    pub fn facts(&self) -> [ProjectionFact<'_, Self>; 4] {
        [
            ProjectionFact::Preserved {
                obligation: text("full-original-request-context"),
            },
            ProjectionFact::Transformed {
                obligation: text("source-profile-eligibility-and-stability"),
                law: text("audio/owned-resonator-prepare-started@1"),
            },
            ProjectionFact::Transformed {
                obligation: text("declared-q30-q14-rounding-policy"),
                law: text("audio/source-nine-term-polynomial-q14@1"),
            },
            ProjectionFact::Lost {
                obligation: text("continuous-acoustic-coefficient-interpretation"),
                class: ProjectionLoss::Approximation,
                detail: &self.loss,
                native_fact: None,
            },
        ]
    }
}
impl ProjectionDomain for ResonatorNumericProjection<'_> {
    type Source = ResonatorNumericReceipt;
    type Target = ResonatorNumericTarget;
    type Detail = ResonatorPolynomialLoss;
    fn source_contract(&self) -> ProjectionText<'_> {
        text("audio/source-bound-resonator-preparation@1")
    }
    fn target_contract(&self) -> ProjectionText<'_> {
        text("audio/polynomial-q30-to-q14-coefficients@1")
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
        if !core::ptr::eq(source, self.receipt)
            || target != Some(&self.target)
            || !native.is_empty()
            || !scores.is_empty()
            || mechanism != ProjectionMechanism::Completed
        {
            return false;
        }
        let expected = self.facts();
        facts.len() == 4
            && facts.iter().zip(&expected).all(|(a, b)| match (a, b) {
                (
                    ProjectionFact::Preserved { obligation: a },
                    ProjectionFact::Preserved { obligation: b },
                ) => a == b,
                (
                    ProjectionFact::Transformed {
                        obligation: a,
                        law: al,
                    },
                    ProjectionFact::Transformed {
                        obligation: b,
                        law: bl,
                    },
                ) => a == b && al == bl,
                (
                    ProjectionFact::Lost {
                        obligation: a,
                        class: ac,
                        detail: ad,
                        native_fact: an,
                    },
                    ProjectionFact::Lost {
                        obligation: b,
                        class: bc,
                        detail: bd,
                        native_fact: bn,
                    },
                ) => a == b && ac == bc && ad == bd && an == bn,
                _ => false,
            })
    }
}
pub struct ResonatorPolynomialPolicy;
impl ProjectionPolicy<ResonatorNumericProjection<'_>> for ResonatorPolynomialPolicy {
    fn identity(&self) -> ProjectionText<'_> {
        text("audio/explicit-polynomial-q30-q14@1")
    }
    fn permits(
        &self,
        _: &ResonatorNumericReceipt,
        _: &ResonatorNumericTarget,
        facts: &[ProjectionFact<'_, ResonatorNumericProjection<'_>>],
    ) -> bool {
        facts.len() == 4
            && matches!(&facts[3],ProjectionFact::Lost{class:ProjectionLoss::Approximation,detail,..} if detail.certified_per_request_bound().is_none())
    }
}
