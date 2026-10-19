//! Encode the full symbolic physical frame and the separately named strategy.
use super::*;
use conduit_core::projection::*;
impl PreparedNearEncoding {
    pub fn encode_mathematical(
        &mut self,
        receipt: &BorrowedMathematicalQuantityBinary32<'_>,
        requested_limit: Quantity,
        limit: RationalQuantity,
        report: &ProjectionReport<'_, MathematicalQuantityBinary32Domain<'_>>,
    ) -> Result<&[u8], StructuredInfoRefusal> {
        if receipt.midpoint().numeric().rounding() != Binary32Rounding::NearestTiesEven {
            return Err(StructuredInfoRefusal::WrongType);
        }
        let codec =
            MathematicalQuantityBinary32EvidenceCodec::new(MathematicalQuantityBinary32Policy {
                maximum_error: limit,
                rounding: receipt.midpoint().numeric().rounding(),
            });
        let length = encode_projection_evidence(
            report,
            &codec,
            MATHEMATICAL_NEAR_PROJECTION_EVIDENCE_LIMITS,
            &mut self.projection_bytes,
        )
        .map_err(|_| StructuredInfoRefusal::WrongType)?;
        let evidence = self
            .projection_evidence
            .as_mut()
            .ok_or(StructuredInfoRefusal::WrongType)?
            .leaf(&self.projection_bytes[..length])?;
        let source = receipt.source();
        let error = receipt.error().maximum();
        let claim = match receipt.error() {
            Binary32ErrorClaim::ExactDistance(_) => 0,
            Binary32ErrorClaim::CertifiedBound(_) => 1,
        };
        let mut error_bytes = [0; 2 + EXACT_RATIONAL_MAX_ENCODED_BYTES];
        let mut limit_bytes = [0; 2 + EXACT_RATIONAL_MAX_ENCODED_BYTES];
        let write = |q: ExactRational, bytes: &mut [u8]| {
            MathematicalValue::rational(q)
                .encode(bytes)
                .map_err(|_| StructuredInfoRefusal::WrongType)
        };
        let error_len = write(error, &mut error_bytes)?;
        let limit_len = write(limit.coordinate(), &mut limit_bytes)?;
        let source_value = self.source.leaf(source.coordinate().encoded())?;
        let wire_value = self.wire.leaf(source.encoded())?;
        let requested = self.requested_limit.leaf(&requested_limit.encode())?;
        let unit = self.unit.leaf(&source.unit().encode())?;
        let error = self.error.leaf(&error_bytes[..error_len])?;
        let limit = self.limit.leaf(&limit_bytes[..limit_len])?;
        let target = self
            .target
            .leaf(&receipt.target().coordinate().bits().to_le_bytes())?;
        let floating = self.floating.record(&[
            validate_canonical_structured_value(evidence)?,
            validate_canonical_structured_value(&self.rounding)?,
            validate_canonical_structured_value(target)?,
        ])?;
        let representation = self.representation.variant(
            "enclosed_binary32",
            validate_canonical_structured_value(floating)?,
        )?;
        let role = match source.role() {
            QuantityRole::Linear => 0,
            QuantityRole::Point => 1,
            QuantityRole::Delta => 2,
        };
        self.result.record(&[
            validate_canonical_structured_value(&self.claims[claim])?,
            validate_canonical_structured_value(limit)?,
            validate_canonical_structured_value(error)?,
            validate_canonical_structured_value(representation)?,
            validate_canonical_structured_value(requested)?,
            validate_canonical_structured_value(&self.roles[role])?,
            validate_canonical_structured_value(source_value)?,
            validate_canonical_structured_value(wire_value)?,
            validate_canonical_structured_value(unit)?,
        ])
    }
}

/// Canonical Type bytes occur once; child values contribute only node framing.
/// This bound uses all domain maximum payloads, not the observed small examples.
pub(super) fn maximum_record_bytes(
    ty: &StructuredInfoType,
) -> Result<usize, StructuredInfoRefusal> {
    fn leaf(payload: usize) -> usize {
        5 + payload
    }
    fn tag(name: &str, payload: usize) -> usize {
        5 + name.len() + payload
    }
    fn record(fields: &[(&str, usize)]) -> usize {
        5 + fields
            .iter()
            .map(|(name, value)| 4 + name.len() + value)
            .sum::<usize>()
    }
    let floating = record(&[
        (
            "projection-evidence",
            leaf(MATHEMATICAL_NEAR_PROJECTION_EVIDENCE_MAX_BYTES),
        ),
        ("rounding", tag("nearest_ties_even", leaf(0))),
        ("target", leaf(4)),
    ]);
    let body = record(&[
        ("claim", tag("certified_bound", leaf(0))),
        ("limit", leaf(2 + EXACT_RATIONAL_MAX_ENCODED_BYTES)),
        ("maximum-error", leaf(2 + EXACT_RATIONAL_MAX_ENCODED_BYTES)),
        ("representation", tag("enclosed_binary32", floating)),
        ("requested-limit", leaf(QUANTITY_ENCODED_LEN)),
        ("role", tag("linear", leaf(0))),
        ("source", leaf(MATHEMATICAL_VALUE_MAX_ENCODED_BYTES)),
        ("source-wire", leaf(MATHEMATICAL_QUANTITY_MAX_ENCODED_BYTES)),
        ("unit", leaf(UNIT_ENCODED_LEN)),
    ]);
    Ok(ty.canonical_bytes()?.len() + body)
}
