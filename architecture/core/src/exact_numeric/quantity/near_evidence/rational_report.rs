//! Replay the actual native/rational producer sequence before comparing its CPE.
//! Producer work is distinct from the consuming validation envelope.
use super::*;
use crate::projection::*;

pub(super) fn verify(
    value: Value<'_>,
    source: RationalQuantity,
    requested_wire: crate::Quantity,
    requested: RationalQuantity,
    representation: VerifiedNearRepresentation,
    work: &mut ExactNumericWork,
) -> Result<(), Refusal> {
    let wire = leaf(field(value, "source-wire")?)?;
    let encoded = leaf(field(value, "projection-evidence")?)?;
    let maximum_error = number(field(value, "maximum-error")?, work)?;
    let claim = field(value, "claim")?
        .variant_tag()
        .map_err(|_| Refusal::Malformed)?;
    let expected_error = match claim {
        "exact_distance" => Binary32ErrorClaim::ExactDistance(maximum_error),
        "certified_bound" => Binary32ErrorClaim::CertifiedBound(maximum_error),
        _ => return Err(Refusal::InvalidClaim),
    };
    if work.remaining() < MATHEMATICAL_NEAR_PROJECTION_WORK {
        return Err(Refusal::WorkLimit);
    }
    let mut replay = ExactNumericWork::new(MATHEMATICAL_NEAR_PROJECTION_WORK);
    let result = (|| {
        // This order matches the actual producer, including the work consumed
        // before projection admission. Requested Quantity preparation happened
        // before Play and is deliberately outside this producer meter.
        let view = crate::QuantityValueProfile::Exact
            .decode(crate::QUANTITY_INFO_ID, wire, &mut replay)
            .map_err(source_refusal)?;
        let replayed = view.rational(&mut replay).map_err(|why| match why {
            crate::QuantityValueRefusal::Rational(why) => rational_error(why),
            _ => Refusal::InvalidSource,
        })?;
        if replayed != source {
            return Err(Refusal::SourceMismatch);
        }
        let limit = effective_limit(requested, source, &mut replay)?;
        match representation {
            VerifiedNearRepresentation::ExactRational => {
                let domain = ExactNearProjectionDomain;
                let policy = ExactNearProjectionPolicy {
                    maximum_error: limit,
                };
                let ids = NearProjectionContentIds::new(wire, source, requested_wire, None);
                let facts = domain.facts();
                let report = ProjectionReport::new(
                    &domain,
                    &policy,
                    ids.input(&domain, &source, &source, &facts),
                )
                .map_err(|_| Refusal::InvalidClaim)?;
                report
                    .require_bound_policy(policy.identity(), policy.configuration_digest().unwrap())
                    .map_err(|_| Refusal::InsufficientTolerance)?;
                verify_projection_evidence(
                    &report,
                    &ExactNearEvidenceCodec::new(policy),
                    RATIONAL_NEAR_PROJECTION_EVIDENCE_LIMITS,
                    encoded,
                )
                .map_err(Refusal::ProjectionEvidence)
            }
            VerifiedNearRepresentation::Binary32 { target, rounding } => {
                let receipt = PreparedQuantityBinary32Projection::prepare(
                    source,
                    rounding,
                    limit,
                    &mut replay,
                )
                .map_err(projection_error)?;
                if receipt.target().coordinate() != target || receipt.error() != expected_error {
                    return Err(Refusal::InvalidClaim);
                }
                let domain = QuantityBinary32ProjectionDomain;
                let policy = QuantityBinary32ProjectionPolicy {
                    maximum_error: limit,
                    rounding,
                };
                let ids =
                    NearProjectionContentIds::new(wire, source, requested_wire, Some(&receipt));
                let facts = receipt.facts();
                let target = receipt.target();
                let report = ProjectionReport::new(
                    &domain,
                    &policy,
                    ids.input(&domain, &receipt, &target, &facts),
                )
                .map_err(|_| Refusal::InvalidClaim)?;
                report
                    .require_bound_policy(policy.identity(), policy.configuration_digest().unwrap())
                    .map_err(|_| Refusal::InsufficientTolerance)?;
                verify_projection_evidence(
                    &report,
                    &QuantityBinary32EvidenceCodec::new(policy),
                    RATIONAL_NEAR_PROJECTION_EVIDENCE_LIMITS,
                    encoded,
                )
                .map_err(Refusal::ProjectionEvidence)
            }
        }
    })();
    // Failed replay is charged too. No retry with a substituted allowance.
    work.charge(replay.spent()).map_err(numeric_error)?;
    result
}
fn source_refusal(why: crate::QuantityValueContractRefusal) -> Refusal {
    use crate::{QuantityValueContractRefusal as C, QuantityValueRefusal as V};
    match why {
        C::Encoding(V::Rational(RationalQuantityRefusal::Numeric(n))) => numeric_error(n),
        C::WrongFamily | C::WrongRole | C::WrongDimension { .. } => Refusal::Physical,
        C::UnsupportedRepresentation { .. } => Refusal::UnsupportedRepresentation,
        _ => Refusal::InvalidSource,
    }
}
