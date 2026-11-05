//! Emit supported checked predicates as immutable data, never runtime decoders.
use super::*;
use conduit_core::{CheckedValueContract, ValueConstraint};
pub(super) fn emit(
    source: &mut String,
    contract: Option<&CheckedValueContract>,
    name: &str,
    kind: PrimitiveInfoKind,
) -> Result<String, Refusal> {
    let Some(contract) = contract else {
        return Ok("None".into());
    };
    let unsupported =
        || Refusal::UnsupportedType("scalar-image/value-contract-witness-unavailable@1".into());
    if conduit_core::fixed_integer_bytes(kind) == 0 {
        return Err(unsupported());
    }
    if conduit_core::primitive_info_kind(contract.value_kind.as_str()) != Some(kind) {
        return Err(Refusal::InvalidProgram);
    }
    contract
        .validate_definition()
        .map_err(|_| Refusal::InvalidProgram)?;
    let core = "conduit_plot::rust_binding::semantic_core";
    let range = match contract.constraints.as_slice() {
        [] => "None".into(),
        [ValueConstraint::FixedIntegerRange {
            minimum,
            maximum,
            minimum_endpoint,
            maximum_endpoint,
        }] => {
            let bytes = |value: &Option<Vec<u8>>| {
                value
                    .as_ref()
                    .map_or_else(|| "None".into(), |bytes| format!("Some(&{bytes:?})"))
            };
            format!("Some({core}::FixedIntegerRange {{ minimum: {}, maximum: {}, minimum_endpoint: {core}::IntervalEndpoint::{minimum_endpoint:?}, maximum_endpoint: {core}::IntervalEndpoint::{maximum_endpoint:?} }})", bytes(minimum), bytes(maximum))
        }
        _ => return Err(unsupported()),
    };
    source.push_str(&format!("static {name}: {core}::FixedIntegerContractSpec<'static> = {core}::FixedIntegerContractSpec {{ value_kind: {:?}, maximum_bytes: {}, range: {range} }};\n", contract.value_kind.as_str(), contract.maximum_bytes));
    Ok(format!("Some(&{name})"))
}
