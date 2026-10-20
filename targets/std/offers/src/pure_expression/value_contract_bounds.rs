//! Sealed numerical validation meters and retained evidence schema payload.
use conduit_core::*;
use conduit_plot::{PortableExpressionProgram, PreparedPortableExpressionEvaluator};
const FIELDS: [&str; 6] = [
    "maximum-constraints",
    "maximum-numerical-validation-work",
    "retained-near-schema-bytes",
    "canonical-contract-bytes",
    "maximum-validated-input-bytes",
    "maximum-validated-output-bytes",
];
fn schema() -> Result<StructuredInfoType, StructuredInfoRefusal> {
    let integer = StructuredInfoType::leaf(kind_id("value/u64"))?;
    StructuredInfoType::record(
        kind_id("std/expression-value-contract-bounds@1"),
        FIELDS
            .iter()
            .map(|name| StructuredFieldType::new(*name, integer.clone()))
            .collect::<Result<_, _>>()?,
    )
}
pub fn pure_expression_value_contract_bounds_profile() -> KindId {
    schema()
        .expect("fixed predicate bound schema")
        .profile()
        .expect("fixed predicate bound profile")
        .value_kind()
        .clone()
}
pub(super) fn property(
    program: &PortableExpressionProgram,
) -> Result<Option<StructuredConfigurationValue>, StructuredInfoRefusal> {
    if !matches!(
        program.root.operation,
        conduit_plot::PortableExpressionOperation::ValueContract { .. }
    ) {
        return Ok(None);
    }
    let prepared = PreparedPortableExpressionEvaluator::new(program)
        .map_err(|_| StructuredInfoRefusal::MalformedCanonicalEncoding)?;
    let bounds = prepared
        .value_contract_bounds()
        .ok_or(StructuredInfoRefusal::MalformedCanonicalEncoding)?;
    let values = [
        bounds.maximum_constraints,
        bounds.maximum_numerical_work,
        bounds.retained_near_schema_bytes,
        bounds.canonical_contract_bytes,
        bounds.maximum_input_bytes,
        bounds.maximum_output_bytes,
    ];
    let integer = StructuredInfoType::leaf(kind_id("value/u64"))?;
    let ty = schema()?;
    let profile = ty.profile()?.value_kind().clone();
    let value = StructuredInfoValue::record(
        ty,
        FIELDS
            .iter()
            .zip(values)
            .map(|(name, value)| {
                StructuredFieldValue::new(
                    *name,
                    StructuredInfoValue::leaf(integer.clone(), value.to_le_bytes().to_vec())?,
                )
            })
            .collect::<Result<_, _>>()?,
    )?;
    StructuredConfigurationValue::new(profile, value.canonical_bytes()?)
        .map(Some)
        .ok_or(StructuredInfoRefusal::MalformedCanonicalEncoding)
}
