//! A schema supplied by the admitted checked catalogue, never by a request
//! seeking a favorable work class. Canonical validation reuses core's prepared
//! structured validator and its finite primitive-leaf checks.
use super::{CostEvidenceRefusal as Refusal, MAXIMUM_COST_INPUT_BYTES};
use conduit_core::{
    KindId, PreparedStructuredValueValidator, StructuredInfoType, StructuredInfoTypeShape,
    ValuePayload,
};

pub struct CostStructuredInputSchema {
    kind: KindId,
    validator: PreparedStructuredValueValidator,
}
impl CostStructuredInputSchema {
    pub fn admit(value_type: &StructuredInfoType, maximum_bytes: usize) -> Result<Self, Refusal> {
        if maximum_bytes > MAXIMUM_COST_INPUT_BYTES {
            return Err(Refusal::InputLimit);
        }
        let kind = match value_type.shape() {
            StructuredInfoTypeShape::Nominal { schema, .. }
            | StructuredInfoTypeShape::Record { schema, .. }
            | StructuredInfoTypeShape::Variant { schema, .. } => schema.clone(),
            _ => return Err(Refusal::InvalidWorkInput),
        };
        if known_leaf(kind.as_str()) || !known_leaves(value_type) {
            return Err(Refusal::InvalidWorkInput);
        }
        let validator = PreparedStructuredValueValidator::new(value_type, maximum_bytes)
            .map_err(Refusal::Structured)?;
        Ok(Self { kind, validator })
    }
    pub fn kind(&self) -> &KindId {
        &self.kind
    }
    pub fn validate(&self, value: &ValuePayload) -> Result<(), Refusal> {
        if value.value_kind != self.kind {
            return Err(Refusal::InvalidWorkInput);
        }
        self.validator
            .validate(&value.encoded)
            .map_err(Refusal::Structured)
    }
}
pub(super) fn known_leaf(kind: &str) -> bool {
    conduit_core::primitive_info_kind(kind).is_some()
        || conduit_core::parse_quantity_role_info_id(kind).is_some()
}
fn known_leaves(value_type: &StructuredInfoType) -> bool {
    match value_type.shape() {
        StructuredInfoTypeShape::Leaf(kind) => known_leaf(kind.as_str()),
        StructuredInfoTypeShape::Nominal { representation, .. } => known_leaves(representation),
        StructuredInfoTypeShape::Collection { element, .. }
        | StructuredInfoTypeShape::Sequence { element, .. } => known_leaves(element),
        StructuredInfoTypeShape::Record { fields, .. } => {
            fields.iter().all(|field| known_leaves(field.value_type()))
        }
        StructuredInfoTypeShape::Variant { cases, .. } => {
            cases.iter().all(|case| known_leaves(case.payload_type()))
        }
    }
}
