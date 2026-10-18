//! Prepared qualified-result encoding; original wire bytes remain inspectable.
use conduit_core::*;
use conduit_semantic_catalog::{
    near_quantity_type, NearQuantityAlgorithm, NUMERICAL_NEAR_MAX_BYTES,
};

pub struct PreparedNearEncoding {
    source: PreparedStructuredComposer,
    wire: PreparedStructuredComposer,
    requested_limit: PreparedStructuredComposer,
    unit: PreparedStructuredComposer,
    error: PreparedStructuredComposer,
    limit: PreparedStructuredComposer,
    exact: PreparedStructuredComposer,
    target: PreparedStructuredComposer,
    floating: PreparedStructuredComposer,
    representation: PreparedStructuredComposer,
    result: PreparedStructuredComposer,
    roles: [Vec<u8>; 3],
    claims: [Vec<u8>; 2],
    rounding: Vec<u8>,
}
fn field(ty: &StructuredInfoType, name: &str) -> StructuredInfoType {
    let StructuredInfoTypeShape::Record { fields, .. } = ty.shape() else {
        unreachable!("reviewed near record")
    };
    fields
        .iter()
        .find(|f| f.name() == name)
        .unwrap()
        .value_type()
        .clone()
}
fn case(ty: &StructuredInfoType, name: &str) -> StructuredInfoType {
    let StructuredInfoTypeShape::Variant { cases, .. } = ty.shape() else {
        unreachable!("reviewed near variant")
    };
    cases
        .iter()
        .find(|c| c.tag() == name)
        .unwrap()
        .payload_type()
        .clone()
}
fn tag(ty: &StructuredInfoType, name: &str) -> Result<Vec<u8>, StructuredInfoRefusal> {
    StructuredInfoValue::variant(
        ty.clone(),
        name,
        StructuredInfoValue::leaf(StructuredInfoType::leaf(kind_id(EMPTY_INFO_ID))?, vec![])?,
    )?
    .canonical_bytes()
}
fn leaf(
    ty: &StructuredInfoType,
    maximum_payload: usize,
) -> Result<PreparedStructuredComposer, StructuredInfoRefusal> {
    PreparedStructuredComposer::new(ty, ty.canonical_bytes()?.len() + 5 + maximum_payload)
}
impl PreparedNearEncoding {
    pub fn new() -> Result<Self, StructuredInfoRefusal> {
        let ty = near_quantity_type();
        let representation = field(&ty, "representation");
        let floating = case(&representation, "binary32");
        let math = field(&ty, "source");
        let role = field(&ty, "role");
        let claim = field(&ty, "claim");
        Ok(Self {
            source: leaf(&math, 2 + EXACT_RATIONAL_MAX_ENCODED_BYTES)?,
            requested_limit: leaf(&field(&ty, "requested-limit"), QUANTITY_ENCODED_LEN)?,
            wire: leaf(&field(&ty, "source-wire"), QUANTITY_VALUE_MAX_ENCODED_BYTES)?,
            unit: leaf(&field(&ty, "unit"), UNIT_ENCODED_LEN)?,
            error: leaf(
                &field(&ty, "maximum-error"),
                2 + EXACT_RATIONAL_MAX_ENCODED_BYTES,
            )?,
            limit: leaf(&field(&ty, "limit"), 2 + EXACT_RATIONAL_MAX_ENCODED_BYTES)?,
            exact: leaf(
                &case(&representation, "exact_rational"),
                2 + EXACT_RATIONAL_MAX_ENCODED_BYTES,
            )?,
            target: leaf(&field(&floating, "target"), 4)?,
            rounding: tag(&field(&floating, "rounding"), "nearest_ties_even")?,
            floating: PreparedStructuredComposer::new(&floating, 4096)?,
            representation: PreparedStructuredComposer::new(&representation, 8192)?,
            result: PreparedStructuredComposer::new(&ty, NUMERICAL_NEAR_MAX_BYTES as usize)?,
            roles: [
                tag(&role, "linear")?,
                tag(&role, "point")?,
                tag(&role, "delta")?,
            ],
            claims: [
                tag(&claim, "exact_distance")?,
                tag(&claim, "certified_bound")?,
            ],
        })
    }
    pub fn prepared_heap_bytes(&self) -> usize {
        [
            &self.source,
            &self.wire,
            &self.requested_limit,
            &self.unit,
            &self.error,
            &self.limit,
            &self.exact,
            &self.target,
            &self.floating,
            &self.representation,
            &self.result,
        ]
        .iter()
        .map(|c| c.prepared_heap_bytes())
        .sum::<usize>()
            + self
                .roles
                .iter()
                .chain(&self.claims)
                .map(Vec::capacity)
                .sum::<usize>()
            + self.rounding.capacity()
    }
    pub fn encoded(&self) -> &[u8] {
        self.result.encoded()
    }
    pub fn encode(
        &mut self,
        source: RationalQuantity,
        wire: &[u8],
        requested_limit: Quantity,
        limit: RationalQuantity,
        algorithm: NearQuantityAlgorithm,
        projection: Option<&PreparedQuantityBinary32Projection>,
    ) -> Result<&[u8], StructuredInfoRefusal> {
        let (error, claim) = match projection.map(|p| p.error()) {
            Some(Binary32ErrorClaim::ExactDistance(v)) => (v, 0),
            Some(Binary32ErrorClaim::CertifiedBound(v)) => (v, 1),
            None => (ExactRational::ZERO, 0),
        };
        let mut source_bytes = [0; 2 + EXACT_RATIONAL_MAX_ENCODED_BYTES];
        let mut error_bytes = [0; 2 + EXACT_RATIONAL_MAX_ENCODED_BYTES];
        let mut limit_bytes = [0; 2 + EXACT_RATIONAL_MAX_ENCODED_BYTES];
        let write = |q: ExactRational, bytes: &mut [u8]| {
            MathematicalValue::rational(q)
                .encode(bytes)
                .map_err(|_| StructuredInfoRefusal::WrongType)
        };
        let source_len = write(source.coordinate(), &mut source_bytes)?;
        let error_len = write(error, &mut error_bytes)?;
        let limit_len = write(limit.coordinate(), &mut limit_bytes)?;
        let source_value = self.source.leaf(&source_bytes[..source_len])?;
        let wire_value = self.wire.leaf(wire)?;
        let requested_limit_value = self.requested_limit.leaf(&requested_limit.encode())?;
        let unit_value = self.unit.leaf(&source.unit().encode())?;
        let error_value = self.error.leaf(&error_bytes[..error_len])?;
        let limit_value = self.limit.leaf(&limit_bytes[..limit_len])?;
        let representation = match algorithm {
            NearQuantityAlgorithm::ExactRational => {
                if projection.is_some() {
                    return Err(StructuredInfoRefusal::WrongType);
                }
                let exact = self.exact.leaf(&source_bytes[..source_len])?;
                self.representation.variant(
                    "exact_rational",
                    validate_canonical_structured_value(exact)?,
                )?
            }
            NearQuantityAlgorithm::Binary32 => {
                let p = projection.ok_or(StructuredInfoRefusal::WrongType)?;
                let target = self
                    .target
                    .leaf(&p.target().coordinate().bits().to_le_bytes())?;
                let floating = self.floating.record(&[
                    validate_canonical_structured_value(&self.rounding)?,
                    validate_canonical_structured_value(target)?,
                ])?;
                self.representation
                    .variant("binary32", validate_canonical_structured_value(floating)?)?
            }
        };
        let role = match source.role() {
            QuantityRole::Linear => 0,
            QuantityRole::Point => 1,
            QuantityRole::Delta => 2,
        };
        self.result.record(&[
            validate_canonical_structured_value(&self.claims[claim])?,
            validate_canonical_structured_value(limit_value)?,
            validate_canonical_structured_value(error_value)?,
            validate_canonical_structured_value(representation)?,
            validate_canonical_structured_value(requested_limit_value)?,
            validate_canonical_structured_value(&self.roles[role])?,
            validate_canonical_structured_value(source_value)?,
            validate_canonical_structured_value(wire_value)?,
            validate_canonical_structured_value(unit_value)?,
        ])
    }
}
