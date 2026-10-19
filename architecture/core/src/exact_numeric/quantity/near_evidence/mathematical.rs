//! Independent replay of a symbolic physical source and named midpoint strategy.
use super::*;
use crate::{
    BorrowedMathematicalQuantityBinary32, MathematicalEnclosureRequirements,
    MathematicalQuantityView, PreparedMathematicalQuantityBinary32,
};
/// Exact replay budget of the prepared enclosed-midpoint Near profile.
/// Shared with its producer; generic report work metadata cannot be reconstructed
/// from the reader's remaining envelope-validation budget.
pub const MATHEMATICAL_NEAR_PROJECTION_WORK: u64 = 200_000_000;
/// Upper bound for the fixed Near report inventory, including complete domain
/// fields, three facts, exact content identities and the twelve loss counters.
pub const MATHEMATICAL_NEAR_PROJECTION_EVIDENCE_MAX_BYTES: usize = 54_650;
pub const MATHEMATICAL_NEAR_PROJECTION_EVIDENCE_LIMITS:
    crate::projection::ProjectionEvidenceLimits = crate::projection::ProjectionEvidenceLimits {
    source_bytes: crate::MATHEMATICAL_BINARY32_EVIDENCE_SOURCE_MAX_BYTES,
    target_bytes: crate::MATHEMATICAL_BINARY32_EVIDENCE_TARGET_MAX_BYTES,
    detail_bytes: crate::MATHEMATICAL_BINARY32_EVIDENCE_DETAIL_MAX_BYTES,
    policy_bytes: crate::MATHEMATICAL_BINARY32_EVIDENCE_POLICY_MAX_BYTES,
    total_bytes: MATHEMATICAL_NEAR_PROJECTION_EVIDENCE_MAX_BYTES,
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
    projection: BorrowedMathematicalQuantityBinary32<'a>,
    maximum_error: ExactRational,
    source_wire_digest: [u8; 32],
}
impl<'a> VerifiedMathematicalNearQuantity<'a> {
    pub const fn source(&self) -> &MathematicalQuantityView<'a> {
        self.projection.source()
    }
    pub const fn target(&self) -> IeeeF32 {
        self.projection.target().coordinate()
    }
    pub const fn rounding(&self) -> Binary32Rounding {
        self.projection.midpoint().numeric().rounding()
    }
    /// Complete independently replayed receipt under the original producer
    /// allowance and pinned projection work budget. Not execution attestation.
    pub const fn projection_receipt(&self) -> &BorrowedMathematicalQuantityBinary32<'a> {
        &self.projection
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
            MATHEMATICAL_NEAR_PROJECTION_WORK,
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
    /// Validate the source representation once, then dispatch to the exact
    /// native/rational or prepared symbolic numerical verifier. No failed-proof
    /// fallback selects a different algorithm. The entire call shares one meter.
    pub fn verify_admitted(
        &mut self,
        encoded: &[u8],
        quantity_kind: &str,
        expected: Option<[u8; 32]>,
        work: &mut ExactNumericWork,
    ) -> Result<(), Refusal> {
        let mut local = ExactNumericWork::new(work.remaining().min(self.maximum_work));
        let result = (|| {
            let value = self.decode_frame(encoded, &mut local)?;
            let wire = leaf(field(value, "source-wire")?)?;
            match wire.first() {
                Some(3) => self
                    .check_mathematical(value, wire, quantity_kind, expected, &mut local)
                    .map(|_| ()),
                Some(1 | 2) => PreparedNearQuantityVerifier::verify_validated(
                    value,
                    quantity_kind,
                    self.consumer_limit,
                    expected,
                    &mut local,
                )
                .map(|_| ()),
                _ => Err(Refusal::UnsupportedRepresentation),
            }
        })();
        work.charge(local.spent()).map_err(numeric_error)?;
        result
    }
    fn decode_frame<'a>(
        &self,
        encoded: &'a [u8],
        work: &mut ExactNumericWork,
    ) -> Result<Value<'a>, Refusal> {
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
        Ok(value)
    }
    fn check<'a>(
        &mut self,
        encoded: &'a [u8],
        quantity_kind: &str,
        expected: Option<[u8; 32]>,
        work: &mut ExactNumericWork,
    ) -> Result<VerifiedMathematicalNearQuantity<'a>, Refusal> {
        let value = self.decode_frame(encoded, work)?;
        let wire = leaf(field(value, "source-wire")?)?;
        self.check_mathematical(value, wire, quantity_kind, expected, work)
    }
    fn check_mathematical<'a>(
        &mut self,
        value: Value<'a>,
        wire: &'a [u8],
        quantity_kind: &str,
        expected: Option<[u8; 32]>,
        work: &mut ExactNumericWork,
    ) -> Result<VerifiedMathematicalNearQuantity<'a>, Refusal> {
        let frame = crate::QuantityValueProfile::Mathematical
            .decode(quantity_kind, wire, work)
            .map_err(wire_refusal)?;
        let requested_wire = crate::Quantity::decode(leaf(field(value, "requested-limit")?)?)
            .map_err(|_| Refusal::InvalidLimit)?;
        let requested =
            RationalQuantity::from_quantity(requested_wire, work).map_err(rational_error)?;
        let physical = RationalQuantity::new(ExactRational::ZERO, frame.unit(), frame.role())
            .map_err(rational_error)?;
        let limit = effective_limit(requested, physical, work)?;
        if work.remaining() < MATHEMATICAL_NEAR_PROJECTION_WORK {
            return Err(Refusal::WorkLimit);
        }
        let mut replay = ExactNumericWork::new(MATHEMATICAL_NEAR_PROJECTION_WORK);
        let receipt = self
            .projection
            .replay_with_allowance(wire, limit, &mut replay);
        // Failed replay is charged too; no alternate budget or algorithm retries.
        work.charge(replay.spent()).map_err(numeric_error)?;
        let receipt = receipt.map_err(projection_refusal)?;
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
        let ids =
            NearProjectionContentIds::new(wire, physical, requested_wire, Some(receipt.midpoint()));
        let domain = crate::MathematicalQuantityBinary32Domain::new();
        let policy = crate::MathematicalQuantityBinary32Policy {
            maximum_error: limit,
            rounding: self.rounding,
        };
        let facts = receipt.facts();
        let target = receipt.target();
        let report = crate::projection::ProjectionReport::new(
            &domain,
            &policy,
            ids.input(&domain, &receipt, &target, &facts),
        )
        .map_err(|_| Refusal::InvalidClaim)?;
        use crate::projection::ProjectionPolicy;
        report
            .require_bound_policy(policy.identity(), policy.configuration_digest().unwrap())
            .map_err(|_| Refusal::InsufficientTolerance)?;
        let evidence = value
            .record_field("projection-evidence")
            .map_err(|_| Refusal::Malformed)?
            .or(payload
                .record_field("projection-evidence")
                .map_err(|_| Refusal::Malformed)?);
        // Only the historical nine-field/two-field schema can omit this.
        // Current schemas require their root field; embedded-report schemas
        // require that field in their payload. No absent report is invented.
        if let Some(evidence) = evidence {
            crate::projection::verify_projection_evidence(
                &report,
                &crate::MathematicalQuantityBinary32EvidenceCodec::new(policy),
                MATHEMATICAL_NEAR_PROJECTION_EVIDENCE_LIMITS,
                leaf(evidence)?,
            )
            .map_err(Refusal::ProjectionEvidence)?;
        }
        Ok(VerifiedMathematicalNearQuantity {
            projection: receipt,
            maximum_error: claimed,
            source_wire_digest: digest,
        })
    }
}
fn check_claim(
    receipt: &BorrowedMathematicalQuantityBinary32<'_>,
    tag: &str,
    claimed: ExactRational,
    _work: &mut ExactNumericWork,
) -> Result<(), Refusal> {
    let expected = match tag {
        "exact_distance" => Binary32ErrorClaim::ExactDistance(claimed),
        "certified_bound" => Binary32ErrorClaim::CertifiedBound(claimed),
        _ => return Err(Refusal::InvalidClaim),
    };
    if receipt.error() != expected {
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
