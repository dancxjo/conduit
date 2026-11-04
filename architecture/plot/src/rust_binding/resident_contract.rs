//! One immutable fixed-contract emitter shared by resident execution profiles.
use alloc::{format, string::String, vec::Vec};
use conduit_core::{CheckedValueContract, ValueConstraint};
#[derive(Debug)]
pub(crate) enum EmissionRefusal {
    Invalid,
    Unsupported,
}
pub(crate) fn emit(
    source: &mut String,
    contract: &CheckedValueContract,
    name: &str,
) -> Result<(), EmissionRefusal> {
    if conduit_core::primitive_info_kind(contract.value_kind.as_str())
        .is_none_or(|kind| conduit_core::fixed_integer_bytes(kind) == 0)
    {
        return Err(EmissionRefusal::Unsupported);
    }
    contract
        .validate_definition()
        .map_err(|_| EmissionRefusal::Invalid)?;
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
        _ => return Err(EmissionRefusal::Unsupported),
    };
    source.push_str(&format!("static {name}: {core}::FixedIntegerContractSpec<'static> = {core}::FixedIntegerContractSpec {{ value_kind: {:?}, maximum_bytes: {}, range: {range} }};\n", contract.value_kind.as_str(), contract.maximum_bytes));
    Ok(())
}
