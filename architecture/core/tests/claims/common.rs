use conduit_core::claims::*;

pub(super) fn text(s: &str) -> ClaimText<'_> {
    ClaimText::new(s).unwrap()
}
pub(super) fn reason() -> ClaimReason<'static> {
    ClaimReason {
        profile: text("fixture/reason@1"),
        explanation: text("Exact supplied observations determine this decision"),
    }
}
pub(super) fn basis<'a>(
    id: &'a str,
    support: &'a [ClaimSupport<'a>],
    conflicts: &'a [ClaimText<'a>],
) -> ClaimBasis<'a> {
    ClaimBasis {
        identity: text(id),
        producer: text("fixture/producer@1"),
        artifact: text("fixture/artifact@1"),
        support,
        conflicts,
        score: None,
        rationale: reason(),
    }
}
pub(super) fn observation() -> [ClaimSupport<'static>; 1] {
    [ClaimSupport::SourceGeneration {
        source: text("sensor/frame"),
        generation: text("frame/17"),
    }]
}
#[derive(PartialEq, Eq)]
pub(super) struct Object {
    pub(super) track: u64,
    pub(super) generation: u64,
}
#[derive(Debug, PartialEq, Eq)]
pub(super) enum Classification {
    Person,
    Mannequin,
}
pub(super) struct Vision;
impl ClaimDomain for Vision {
    type Target = Object;
    type Value = Classification;
    fn contract(&self) -> ClaimText<'_> {
        text("vision/classification@1")
    }
    fn validate(&self, target: &Object, _: &Classification) -> bool {
        target.generation > 0
    }
}
pub(super) struct ManualPolicy;
impl ClaimPolicy<Vision> for ManualPolicy {
    fn identity(&self) -> ClaimText<'_> {
        text("fixture/manual-correction@1")
    }
    fn assess<'a>(
        &self,
        _: &SemanticClaim<'a, Vision>,
        _: &[&SemanticClaim<'a, Vision>],
    ) -> Option<ClaimReason<'a>> {
        None
    }
    fn decide<'a>(
        &self,
        candidates: &[&SemanticClaim<'a, Vision>],
        eligible: &[bool],
        _: &[&SemanticClaim<'a, Vision>],
    ) -> ClaimDecision<'a> {
        candidates
            .iter()
            .zip(eligible)
            .find(|(c, e)| **e && c.basis().identity.as_str() == "c3/manual")
            .map_or(ClaimDecision::Abstained { reason: reason() }, |(c, _)| {
                ClaimDecision::Selected {
                    identity: c.basis().identity,
                    reason: ClaimReason {
                        profile: text("vision/manual-correction@1"),
                        explanation: text(
                            "Annotation 4 corrects classifications for track 8 in frame 17",
                        ),
                    },
                }
            })
    }
}
pub(super) struct Conservative;
impl<D: ClaimDomain> ClaimPolicy<D> for Conservative {
    fn identity(&self) -> ClaimText<'_> {
        text("fixture/insufficient-evidence@1")
    }
    fn assess<'a>(
        &self,
        _: &SemanticClaim<'a, D>,
        _: &[&SemanticClaim<'a, D>],
    ) -> Option<ClaimReason<'a>> {
        None
    }
    fn decide<'a>(
        &self,
        _: &[&SemanticClaim<'a, D>],
        _: &[bool],
        _: &[&SemanticClaim<'a, D>],
    ) -> ClaimDecision<'a> {
        ClaimDecision::Abstained {
            reason: ClaimReason {
                profile: text("fixture/insufficient-evidence@1"),
                explanation: text(
                    "The supplied evidence does not distinguish the eligible alternatives",
                ),
            },
        }
    }
}
