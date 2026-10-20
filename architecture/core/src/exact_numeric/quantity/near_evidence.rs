//! Borrowed, bounded verification of physical numerical evidence.
//! A record's fields and canonical bytes alone never establish its error claim.
use super::*;
use crate::{
    validate_canonical_structured_value, Binary32ErrorClaim, Binary32Refusal, Binary32Rounding,
    IeeeF32, PreparedQuantityBinary32Projection, QuantityBinary32ProjectionRefusal,
    StructuredInfoType, ValidatedCanonicalStructuredValue as Value,
};
use alloc::vec::Vec;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NearQuantityEvidenceRefusal {
    Schema,
    Malformed,
    Extent,
    InvalidSource,
    SourceMismatch,
    Physical,
    InvalidLimit,
    InvalidClaim,
    UnsupportedDenotation,
    UnsupportedCoordinate,
    UnsupportedRepresentation,
    FiniteRange,
    WorkLimit,
    InsufficientTolerance,
    ExactnessRequired,
    ProjectionEvidence(crate::projection::ProjectionEvidenceRefusal),
}
type Refusal = NearQuantityEvidenceRefusal;
pub const NEAR_QUANTITY_EVIDENCE_MAX_BYTES: usize = 131_072;
mod schema;
use schema::protocol_schema;
mod identities;
mod rational_report;
pub use identities::*;
mod mathematical;
pub use mathematical::*;
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VerifiedNearRepresentation {
    ExactRational,
    Binary32 {
        target: IeeeF32,
        rounding: Binary32Rounding,
    },
}
/// Constructible only after source, representation, actual error and consuming
/// physical/tolerance checks pass. This does not attest a producer's execution.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct VerifiedNearQuantity {
    source: RationalQuantity,
    representation: VerifiedNearRepresentation,
    maximum_error: ExactRational,
    source_wire_digest: [u8; 32],
}
impl VerifiedNearQuantity {
    pub const fn source(&self) -> RationalQuantity {
        self.source
    }
    pub const fn representation(&self) -> VerifiedNearRepresentation {
        self.representation
    }
    /// Absolute coordinate error in source().unit(); its role is a difference.
    pub const fn maximum_error(&self) -> ExactRational {
        self.maximum_error
    }
    /// Representation custody, distinct from physical equality across Units.
    pub const fn source_wire_digest(&self) -> [u8; 32] {
        self.source_wire_digest
    }
    pub fn require_exact_target(&self) -> Result<RationalQuantity, Refusal> {
        if self.maximum_error == ExactRational::ZERO {
            Ok(self.source)
        } else {
            Err(Refusal::ExactnessRequired)
        }
    }
}
pub struct PreparedNearQuantityVerifier {
    schema: Vec<u8>,
}
impl PreparedNearQuantityVerifier {
    /// The caller supplies the exact declared evidence schema. Validation covers
    /// the numerical protocol, not unrelated nominal laws of a custom schema.
    pub fn new(schema: &StructuredInfoType) -> Result<Self, Refusal> {
        if !protocol_schema(schema) {
            return Err(Refusal::Schema);
        }
        Ok(Self {
            schema: schema.canonical_bytes().map_err(|_| Refusal::Schema)?,
        })
    }
    pub fn prepared_heap_bytes(&self) -> usize {
        self.schema.capacity()
    }
    /// expected_source_wire_digest optionally binds an independently retained
    /// source. It must not be confused with cross-Unit mathematical equality.
    pub fn verify(
        &self,
        encoded: &[u8],
        quantity_kind: &str,
        consumer_limit: RationalQuantity,
        expected_source_wire_digest: Option<[u8; 32]>,
        work: &mut ExactNumericWork,
    ) -> Result<VerifiedNearQuantity, Refusal> {
        Self::verify_with_schema(
            &self.schema,
            encoded,
            quantity_kind,
            consumer_limit,
            expected_source_wire_digest,
            work,
        )
    }

    /// A checked value contract retains these schema bytes during preparation.
    /// This borrowed entrance performs no schema construction during Play.
    pub(crate) fn verify_with_schema(
        schema: &[u8],
        encoded: &[u8],
        quantity_kind: &str,
        consumer_limit: RationalQuantity,
        expected_source_wire_digest: Option<[u8; 32]>,
        work: &mut ExactNumericWork,
    ) -> Result<VerifiedNearQuantity, Refusal> {
        if encoded.len() > NEAR_QUANTITY_EVIDENCE_MAX_BYTES {
            return Err(Refusal::Extent);
        }
        let value = validate_canonical_structured_value(encoded).map_err(|_| Refusal::Malformed)?;
        if value.type_bytes() != schema {
            return Err(Refusal::Schema);
        }
        Self::verify_validated(
            value,
            quantity_kind,
            consumer_limit,
            expected_source_wire_digest,
            work,
        )
    }
    // The caller must first validate the complete canonical frame and pin its
    // schema. Shared by the explicit prepared all-form consumer dispatch.
    fn verify_validated(
        value: Value<'_>,
        quantity_kind: &str,
        consumer_limit: RationalQuantity,
        expected_source_wire_digest: Option<[u8; 32]>,
        work: &mut ExactNumericWork,
    ) -> Result<VerifiedNearQuantity, Refusal> {
        let wire = leaf(field(value, "source-wire")?)?;
        let view = crate::QuantityValueProfile::Exact
            .decode(quantity_kind, wire, work)
            .map_err(|why| match why {
                crate::QuantityValueContractRefusal::Encoding(
                    crate::QuantityValueRefusal::Rational(RationalQuantityRefusal::Numeric(
                        ExactNumericRefusal::WorkLimit,
                    )),
                ) => Refusal::WorkLimit,
                crate::QuantityValueContractRefusal::UnsupportedRepresentation { .. } => {
                    Refusal::UnsupportedRepresentation
                }
                crate::QuantityValueContractRefusal::WrongFamily
                | crate::QuantityValueContractRefusal::WrongRole
                | crate::QuantityValueContractRefusal::WrongDimension { .. } => Refusal::Physical,
                _ => Refusal::InvalidSource,
            })?;
        let digest = view.representation_digest();
        if expected_source_wire_digest.is_some_and(|expected| expected != digest) {
            return Err(Refusal::SourceMismatch);
        }
        let source = view.rational(work).map_err(|why| match why {
            crate::QuantityValueRefusal::Rational(why) => rational_error(why),
            _ => Refusal::InvalidSource,
        })?;
        let coordinate = number(field(value, "source")?, work)?;
        let unit = Unit::decode(leaf(field(value, "unit")?)?).map_err(|_| Refusal::Physical)?;
        let role = field(value, "role")?
            .variant_tag()
            .map_err(|_| Refusal::Malformed)?;
        let expected_role = match source.role() {
            QuantityRole::Linear => "linear",
            QuantityRole::Point => "point",
            QuantityRole::Delta => "delta",
        };
        if coordinate != source.coordinate() || unit != source.unit() || role != expected_role {
            return Err(Refusal::SourceMismatch);
        }
        let requested = crate::Quantity::decode(leaf(field(value, "requested-limit")?)?)
            .map_err(|_| Refusal::InvalidLimit)?;
        let requested_wire = requested;
        let requested = RationalQuantity::from_quantity(requested, work).map_err(rational_error)?;
        let limit = effective_limit(requested, source, work)?;
        if number(field(value, "limit")?, work)? != limit.coordinate() {
            return Err(Refusal::InvalidLimit);
        }
        let maximum_error = number(field(value, "maximum-error")?, work)?;
        if maximum_error.is_negative() {
            return Err(Refusal::InvalidClaim);
        }
        if maximum_error
            .checked_compare(limit.coordinate(), work)
            .map_err(numeric_error)?
            == Ordering::Greater
        {
            return Err(Refusal::InvalidClaim);
        }
        let claim = field(value, "claim")?
            .variant_tag()
            .map_err(|_| Refusal::Malformed)?;
        let representation = field(value, "representation")?;
        let representation = match representation
            .variant_tag()
            .map_err(|_| Refusal::Malformed)?
        {
            "exact_rational" => {
                let target = representation
                    .variant_payload("exact_rational")
                    .map_err(|_| Refusal::Malformed)?
                    .ok_or(Refusal::Malformed)?;
                if number(target, work)? != source.coordinate()
                    || maximum_error != ExactRational::ZERO
                    || claim != "exact_distance"
                {
                    return Err(Refusal::InvalidClaim);
                }
                VerifiedNearRepresentation::ExactRational
            }
            "binary32" => {
                let payload = representation
                    .variant_payload("binary32")
                    .map_err(|_| Refusal::Malformed)?
                    .ok_or(Refusal::Malformed)?;
                let target = IeeeF32::from_bits(u32::from_le_bytes(
                    leaf(field(payload, "target")?)?
                        .try_into()
                        .map_err(|_| Refusal::Malformed)?,
                ));
                let rounding = match field(payload, "rounding")?
                    .variant_tag()
                    .map_err(|_| Refusal::Malformed)?
                {
                    "nearest_ties_even" => Binary32Rounding::NearestTiesEven,
                    "toward_zero" => Binary32Rounding::TowardZero,
                    "toward_positive" => Binary32Rounding::TowardPositive,
                    "toward_negative" => Binary32Rounding::TowardNegative,
                    _ => return Err(Refusal::UnsupportedRepresentation),
                };
                // New records are checked once by original-policy report replay
                // below. Historical records have no retained report to replay.
                if value
                    .record_field("projection-evidence")
                    .map_err(|_| Refusal::Malformed)?
                    .is_none()
                {
                    let allowance =
                        RationalQuantity::new(maximum_error, source.unit(), limit.role())
                            .map_err(rational_error)?;
                    let checked = PreparedQuantityBinary32Projection::prepare(
                        source, rounding, allowance, work,
                    )
                    .map_err(projection_error)?;
                    if checked.target().coordinate() != target {
                        return Err(Refusal::InvalidClaim);
                    }
                    if claim == "exact_distance" {
                        if checked.error() != Binary32ErrorClaim::ExactDistance(maximum_error) {
                            return Err(Refusal::InvalidClaim);
                        }
                    } else if claim != "certified_bound" {
                        return Err(Refusal::InvalidClaim);
                    }
                }
                VerifiedNearRepresentation::Binary32 { target, rounding }
            }
            _ => return Err(Refusal::UnsupportedRepresentation),
        };
        if value
            .record_field("projection-evidence")
            .map_err(|_| Refusal::Malformed)?
            .is_some()
        {
            rational_report::verify(
                value,
                source,
                requested_wire,
                requested,
                representation,
                work,
            )?;
        }
        // Physical compatibility is mandatory even when the target is exact.
        let consumer = effective_limit(consumer_limit, source, work)?;
        if maximum_error
            .checked_compare(consumer.coordinate(), work)
            .map_err(numeric_error)?
            == Ordering::Greater
        {
            return Err(Refusal::InsufficientTolerance);
        }
        Ok(VerifiedNearQuantity {
            source,
            representation,
            maximum_error,
            source_wire_digest: digest,
        })
    }
}
fn effective_limit(
    limit: RationalQuantity,
    source: RationalQuantity,
    work: &mut ExactNumericWork,
) -> Result<RationalQuantity, Refusal> {
    if limit.coordinate().is_negative() || limit.role() == QuantityRole::Point {
        return Err(Refusal::InvalidLimit);
    }
    let limit = limit
        .convert_to_unit(source.unit(), work)
        .map_err(rational_error)?;
    let role = if source.role() == QuantityRole::Point {
        QuantityRole::Delta
    } else {
        source.role()
    };
    if limit.role() != role || limit.coordinate().is_negative() {
        return Err(Refusal::Physical);
    }
    Ok(limit)
}
fn field<'a>(value: Value<'a>, name: &str) -> Result<Value<'a>, Refusal> {
    value
        .record_field(name)
        .map_err(|_| Refusal::Malformed)?
        .ok_or(Refusal::Schema)
}
fn leaf(value: Value<'_>) -> Result<&[u8], Refusal> {
    let node = value.value_node();
    if node.first() != Some(&0) || node.len() < 5 {
        return Err(Refusal::Schema);
    }
    let n = u32::from_le_bytes(node[1..5].try_into().unwrap()) as usize;
    if n != node.len() - 5 {
        return Err(Refusal::Malformed);
    }
    Ok(&node[5..])
}
fn number(value: Value<'_>, work: &mut ExactNumericWork) -> Result<ExactRational, Refusal> {
    crate::MathematicalValueView::decode(leaf(value)?, work)
        .map_err(|why| match why {
            crate::MathematicalValueRefusal::Numeric(why) => numeric_error(why),
            crate::MathematicalValueRefusal::NodeBound
            | crate::MathematicalValueRefusal::DepthBound => Refusal::Extent,
            crate::MathematicalValueRefusal::UnsupportedVersion => {
                Refusal::UnsupportedRepresentation
            }
            _ => Refusal::Malformed,
        })?
        .as_rational(work)
        .map_err(|why| match why {
            crate::MathematicalValueRefusal::Numeric(why) => numeric_error(why),
            _ => Refusal::Malformed,
        })?
        .ok_or(Refusal::UnsupportedDenotation)
}
fn numeric_error(why: ExactNumericRefusal) -> Refusal {
    match why {
        ExactNumericRefusal::WorkLimit => Refusal::WorkLimit,
        ExactNumericRefusal::ExtentExceeded => Refusal::Extent,
        _ => Refusal::InvalidClaim,
    }
}
fn rational_error(why: RationalQuantityRefusal) -> Refusal {
    match why {
        RationalQuantityRefusal::Numeric(why) => numeric_error(why),
        RationalQuantityRefusal::Physical(crate::QuantityConversionRefusal::Inexact) => {
            Refusal::UnsupportedCoordinate
        }
        RationalQuantityRefusal::Physical(crate::QuantityConversionRefusal::Overflow) => {
            Refusal::Extent
        }
        _ => Refusal::Physical,
    }
}
fn projection_error(why: QuantityBinary32ProjectionRefusal) -> Refusal {
    match why {
        QuantityBinary32ProjectionRefusal::PhysicalToleranceMismatch => Refusal::Physical,
        QuantityBinary32ProjectionRefusal::Numeric(Binary32Refusal::Numeric(
            ExactNumericRefusal::OutOfRange,
        )) => Refusal::FiniteRange,
        QuantityBinary32ProjectionRefusal::Numeric(Binary32Refusal::Numeric(why)) => {
            numeric_error(why)
        }
        _ => Refusal::InvalidClaim,
    }
}

#[cfg(test)]
mod tests;
