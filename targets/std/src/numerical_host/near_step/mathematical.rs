//! The ordinary Near lifecycle with an explicitly selected mathematical profile.
use super::*;
pub(crate) fn requirements() -> MathematicalEnclosureRequirements {
    let mut work = ExactNumericWork::new(NEAR_NUMERICAL_WORK);
    MathematicalEnclosureRequirements {
        maximum_width: ExactRational::ratio(1, 100_000_000_000_000_000, &mut work).unwrap(),
        maximum_leaf_width: ExactRational::ratio(1, 100_000_000_000_000_000_000, &mut work)
            .unwrap(),
        maximum_constant_terms: 32,
        maximum_root_refinements: 64,
        maximum_scratch_payload_bytes: 131072,
        maximum_retained_payload_bytes: 131072,
    }
}
impl NearQuantityBack {
    pub(super) fn evaluate_mathematical(&mut self, bytes: &[u8]) -> Result<(), NearRefusal> {
        let mut work = ExactNumericWork::new(NEAR_NUMERICAL_WORK);
        let p = self
            .state
            .mathematical
            .as_mut()
            .ok_or(NearRefusal::Lifecycle)?
            .execute(bytes, &mut work)
            .map_err(refusal)?;
        let source =
            RationalQuantity::new(ExactRational::ZERO, p.source().unit(), p.source().role())
                .map_err(rational)?;
        // This profile pins the allowance's Unit during preparation. Numerical
        // realization cannot silently apply a coordinate-conversion law.
        let limit = self.state.limit;
        let ids = ProjectionIds::new(
            bytes,
            source,
            self.state.requested_limit,
            Some(p.midpoint()),
        );
        let facts = p.facts();
        let target = p.target();
        let policy = MathematicalQuantityBinary32Policy {
            maximum_error: limit,
            rounding: Binary32Rounding::NearestTiesEven,
        };
        let domain = MathematicalQuantityBinary32Domain::new();
        let report = checked_report(&domain, &policy, &p, &target, &facts, &ids)?;
        self.state
            .encoding
            .encode_mathematical(&p, self.state.requested_limit, limit, &report)
            .map_err(|_| NearRefusal::Encoding)?;
        Ok(())
    }
}
fn refusal(why: MathematicalQuantityBinary32Refusal) -> NearRefusal {
    use MathematicalQuantityBinary32Refusal as R;
    match why {
        R::Numeric(why) => numeric(why),
        R::Rounding(why) => projection(why),
        R::PhysicalToleranceMismatch | R::NegativeTolerance => NearRefusal::Physical,
        R::ToleranceExceeded => NearRefusal::Tolerance,
        R::RetainedStorageLimit => NearRefusal::Extent,
        R::Enclosure(MathematicalQuantityEnclosureRefusal::Numeric(why)) => numeric(why),
        R::Enclosure(MathematicalQuantityEnclosureRefusal::Enclosure(
            MathematicalEnclosureRefusal::Numeric(why),
        )) => numeric(why),
        R::Enclosure(_) => NearRefusal::UnsupportedCoordinate,
    }
}
