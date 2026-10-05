use conduit_core::{
    claims::{ClaimScore, ClaimText},
    projection::*,
};
pub fn text(s: &str) -> ProjectionText<'_> {
    ProjectionText::new(s).unwrap()
}
pub struct Bits;
impl ProjectionDomain for Bits {
    type Source = u64;
    type Target = u64;
    type Detail = usize;
    fn source_contract(&self) -> ProjectionText<'_> {
        text("bits@1")
    }
    fn target_contract(&self) -> ProjectionText<'_> {
        text("subset@1")
    }
    fn validate(
        &self,
        source: &u64,
        target: Option<&u64>,
        facts: &[ProjectionFact<'_, Self>],
        _: &[ProjectionNativeFact<'_>],
        _: &[ProjectionScore<'_>],
        mechanism: ProjectionMechanism,
    ) -> bool {
        if mechanism != ProjectionMechanism::Completed {
            return target.is_none() && facts.is_empty();
        }
        let mut covered = 0;
        let mut preserved = 0;
        for fact in facts {
            let Ok(bit) = fact.obligation().as_str().parse::<usize>() else {
                return false;
            };
            if bit >= 64 || source & (1 << bit) == 0 {
                return false;
            }
            covered |= 1 << bit;
            match fact {
                ProjectionFact::Preserved { .. } => preserved |= 1 << bit,
                ProjectionFact::Transformed { law, .. } if law.as_str() == "identity-layout@1" => {
                    preserved |= 1 << bit
                }
                ProjectionFact::Lost { detail, .. } if **detail == bit => (),
                _ => return false,
            }
        }
        covered == *source && target.is_none_or(|t| *t == preserved)
    }
}
pub struct Budget(pub usize);
impl ProjectionPolicy<Bits> for Budget {
    fn identity(&self) -> ProjectionText<'_> {
        if self.0 == 0 {
            text("strict@1")
        } else {
            text("loss-budget@1")
        }
    }
    fn permits(&self, _: &u64, _: &u64, facts: &[ProjectionFact<'_, Bits>]) -> bool {
        facts
            .iter()
            .filter(|f| matches!(f, ProjectionFact::Lost { .. }))
            .count()
            <= self.0
    }
}
pub fn input<'a>(
    source: &'a u64,
    target: Option<&'a u64>,
    facts: &'a [ProjectionFact<'a, Bits>],
) -> ProjectionInput<'a, Bits> {
    ProjectionInput {
        basis: ProjectionBasis {
            report: text("report/1"),
            source: text("source/1"),
            target: target.map(|_| text("target/1")),
            projector: text("projector@1"),
            requested_route: text("subset@1"),
            boundary: None,
        },
        source,
        target,
        facts,
        native: &[],
        scores: &[],
        admitted: &[],
        attempts: &[],
        selected_attempt: None,
        mechanism: ProjectionMechanism::Completed,
        diagnostics: &[],
    }
}
pub fn score<'a>(scale: &'a str, producer: &'a str, value: i64) -> ClaimScore<'a> {
    ClaimScore::new(
        ClaimText::new(scale).unwrap(),
        None,
        ClaimText::new(producer).unwrap(),
        0,
        100,
        value,
    )
    .unwrap()
}
