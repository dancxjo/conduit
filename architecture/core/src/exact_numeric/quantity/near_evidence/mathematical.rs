//! Independent replay of a symbolic physical source and named midpoint strategy.
use super::*;
use crate::{
    BorrowedMathematicalQuantityBinary32, MathematicalEnclosureRequirements,
    MathematicalQuantityView, PreparedMathematicalQuantityBinary32,
};
pub const MATHEMATICAL_NEAR_VALIDATION_WORK: u64 = 32 * crate::MATHEMATICAL_VALUE_VALIDATION_WORK;
pub struct PreparedMathematicalNearQuantityVerifier {
    schema: Vec<u8>,
    projection: PreparedMathematicalQuantityBinary32,
    consumer_limit: RationalQuantity,
    rounding: Binary32Rounding,
    maximum_work: u64,
}
/// Verified value/error data. This does not attest the supplier's execution.
pub struct VerifiedMathematicalNearQuantity<'a> {
    source: MathematicalQuantityView<'a>,
    target: IeeeF32,
    rounding: Binary32Rounding,
    maximum_error: ExactRational,
    source_wire_digest: [u8; 32],
}
impl<'a> VerifiedMathematicalNearQuantity<'a> {
    pub const fn source(&self) -> &MathematicalQuantityView<'a> {
        &self.source
    }
    pub const fn target(&self) -> IeeeF32 {
        self.target
    }
    pub const fn rounding(&self) -> Binary32Rounding {
        self.rounding
    }
    pub const fn maximum_error(&self) -> ExactRational {
        self.maximum_error
    }
    pub const fn source_wire_digest(&self) -> [u8; 32] {
        self.source_wire_digest
    }
}
impl PreparedMathematicalNearQuantityVerifier {
    /// Pin the exact output schema, enclosure profile, rounding and consuming
    /// physical allowance before execution. Different enclosures are not assumed
    /// to have identical midpoint rounding results.
    pub fn new(
        schema: &StructuredInfoType,
        requirements: MathematicalEnclosureRequirements,
        rounding: Binary32Rounding,
        consumer_limit: RationalQuantity,
        maximum_work: u64,
    ) -> Result<Self, Refusal> {
        if !protocol_schema(schema) {
            return Err(Refusal::Schema);
        }
        let schema = schema.canonical_bytes().map_err(|_| Refusal::Schema)?;
        if schema.len() > 8192 {
            return Err(Refusal::Schema);
        }
        let projection = PreparedMathematicalQuantityBinary32::new(
            requirements,
            rounding,
            consumer_limit,
            maximum_work,
        )
        .map_err(|_| Refusal::InvalidLimit)?;
        Ok(Self {
            schema,
            projection,
            consumer_limit,
            rounding,
            maximum_work,
        })
    }
    pub fn prepared_heap_bytes(&self) -> usize {
        self.schema.capacity() + self.projection.retained_payload_bytes()
            - core::mem::size_of::<PreparedMathematicalQuantityBinary32>()
    }
    pub fn verify<'a>(
        &mut self,
        encoded: &'a [u8],
        quantity_kind: &str,
        expected_source_wire_digest: Option<[u8; 32]>,
        work: &mut ExactNumericWork,
    ) -> Result<VerifiedMathematicalNearQuantity<'a>, Refusal> {
        let mut local = ExactNumericWork::new(work.remaining().min(self.maximum_work));
        let result = self.check(
            encoded,
            quantity_kind,
            expected_source_wire_digest,
            &mut local,
        );
        work.charge(local.spent()).map_err(numeric_error)?;
        result
    }
    fn check<'a>(
        &mut self,
        encoded: &'a [u8],
        quantity_kind: &str,
        expected: Option<[u8; 32]>,
        work: &mut ExactNumericWork,
    ) -> Result<VerifiedMathematicalNearQuantity<'a>, Refusal> {
        if encoded.len() > NEAR_QUANTITY_EVIDENCE_MAX_BYTES {
            return Err(Refusal::Extent);
        }
        // Reserve a finite bound for all generic structured mathematical-leaf
        // validation before invoking that independently bounded parser.
        work.charge(MATHEMATICAL_NEAR_VALIDATION_WORK)
            .map_err(numeric_error)?;
        let value = validate_canonical_structured_value(encoded).map_err(|_| Refusal::Malformed)?;
        if value.type_bytes() != self.schema {
            return Err(Refusal::Schema);
        }
        let wire = leaf(field(value, "source-wire")?)?;
        crate::QuantityValueProfile::Mathematical
            .decode(quantity_kind, wire, work)
            .map_err(wire_refusal)?;
        let receipt = self
            .projection
            .execute(wire, work)
            .map_err(projection_refusal)?;
        let source = *receipt.source();
        let digest = source.representation_digest();
        if expected.is_some_and(|pin| pin != digest) {
            return Err(Refusal::SourceMismatch);
        }
        let role = match source.role() {
            QuantityRole::Linear => "linear",
            QuantityRole::Point => "point",
            QuantityRole::Delta => "delta",
        };
        if leaf(field(value, "source")?)? != source.coordinate().encoded()
            || Unit::decode(leaf(field(value, "unit")?)?).map_err(|_| Refusal::Physical)?
                != source.unit()
            || field(value, "role")?
                .variant_tag()
                .map_err(|_| Refusal::Malformed)?
                != role
        {
            return Err(Refusal::SourceMismatch);
        }
        let requested = crate::Quantity::decode(leaf(field(value, "requested-limit")?)?)
            .map_err(|_| Refusal::InvalidLimit)?;
        let requested = RationalQuantity::from_quantity(requested, work).map_err(rational_error)?;
        let physical = RationalQuantity::new(ExactRational::ZERO, source.unit(), source.role())
            .map_err(rational_error)?;
        let limit = effective_limit(requested, physical, work)?;
        if number(field(value, "limit")?, work)? != limit.coordinate() {
            return Err(Refusal::InvalidLimit);
        }
        let claimed = number(field(value, "maximum-error")?, work)?;
        if claimed.is_negative()
            || claimed
                .checked_compare(limit.coordinate(), work)
                .map_err(numeric_error)?
                == Ordering::Greater
        {
            return Err(Refusal::InvalidClaim);
        }
        if claimed
            .checked_compare(self.consumer_limit.coordinate(), work)
            .map_err(numeric_error)?
            == Ordering::Greater
        {
            return Err(Refusal::InsufficientTolerance);
        }
        let representation = field(value, "representation")?;
        let payload = representation
            .variant_payload("enclosed_binary32")
            .map_err(|_| Refusal::Malformed)?
            .ok_or(Refusal::UnsupportedRepresentation)?;
        let target = IeeeF32::from_bits(u32::from_le_bytes(
            leaf(field(payload, "target")?)?
                .try_into()
                .map_err(|_| Refusal::Malformed)?,
        ));
        let rounding = field(payload, "rounding")?
            .variant_tag()
            .map_err(|_| Refusal::Malformed)?;
        let expected_rounding = match self.rounding {
            Binary32Rounding::NearestTiesEven => "nearest_ties_even",
            Binary32Rounding::TowardZero => "toward_zero",
            Binary32Rounding::TowardPositive => "toward_positive",
            Binary32Rounding::TowardNegative => "toward_negative",
        };
        if rounding != expected_rounding || target != receipt.target().coordinate() {
            return Err(Refusal::InvalidClaim);
        }
        check_claim(
            &receipt,
            field(value, "claim")?
                .variant_tag()
                .map_err(|_| Refusal::Malformed)?,
            claimed,
            work,
        )?;
        Ok(VerifiedMathematicalNearQuantity {
            source,
            target,
            rounding: self.rounding,
            maximum_error: claimed,
            source_wire_digest: digest,
        })
    }
}
fn check_claim(
    receipt: &BorrowedMathematicalQuantityBinary32<'_>,
    tag: &str,
    claimed: ExactRational,
    work: &mut ExactNumericWork,
) -> Result<(), Refusal> {
    if tag == "exact_distance" {
        if receipt.error() != Binary32ErrorClaim::ExactDistance(claimed) {
            return Err(Refusal::InvalidClaim);
        }
    } else if tag != "certified_bound"
        || receipt
            .error()
            .maximum()
            .checked_compare(claimed, work)
            .map_err(numeric_error)?
            == Ordering::Greater
    {
        return Err(Refusal::InvalidClaim);
    }
    Ok(())
}
fn projection_refusal(why: crate::MathematicalQuantityBinary32Refusal) -> Refusal {
    use crate::MathematicalQuantityBinary32Refusal as R;
    match why {
        R::Numeric(why) => numeric_error(why),
        R::Rounding(QuantityBinary32ProjectionRefusal::Numeric(
            Binary32Refusal::ToleranceExceeded,
        )) => Refusal::InsufficientTolerance,
        R::Rounding(why) => projection_error(why),
        R::PhysicalToleranceMismatch => Refusal::Physical,
        R::NegativeTolerance => Refusal::InvalidLimit,
        R::ToleranceExceeded => Refusal::InsufficientTolerance,
        R::RetainedStorageLimit => Refusal::Extent,
        R::Enclosure(crate::MathematicalQuantityEnclosureRefusal::Numeric(why)) => {
            numeric_error(why)
        }
        R::Enclosure(crate::MathematicalQuantityEnclosureRefusal::Enclosure(
            crate::MathematicalEnclosureRefusal::Numeric(why),
        )) => numeric_error(why),
        R::Enclosure(crate::MathematicalQuantityEnclosureRefusal::Quantity(
            crate::MathematicalQuantityRefusal::Numeric(why),
        )) => numeric_error(why),
        R::Enclosure(_) => Refusal::InvalidClaim,
    }
}

fn wire_refusal(why: crate::QuantityValueContractRefusal) -> Refusal {
    use crate::{
        MathematicalQuantityRefusal as M, MathematicalValueRefusal as S,
        QuantityValueContractRefusal as C, QuantityValueRefusal as V,
    };
    match why {
        C::Encoding(V::Mathematical(M::Numeric(n)))
        | C::Encoding(V::Mathematical(M::Coordinate(S::Numeric(n)))) => numeric_error(n),
        C::WrongFamily | C::WrongRole | C::WrongDimension { .. } => Refusal::Physical,
        C::UnsupportedRepresentation { .. } => Refusal::UnsupportedRepresentation,
        _ => Refusal::InvalidSource,
    }
}
