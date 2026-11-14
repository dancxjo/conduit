//! One immutable fixed-contract emitter shared by resident execution profiles.
use alloc::{format, string::String};
use conduit_core::{
    BorrowedFixedIntegerContract, CheckedValueContract, FixedIntegerContractSpec,
    FixedIntegerRange, ValueConstraint,
};
#[derive(Debug)]
pub(crate) enum EmissionRefusal {
    Invalid,
    Unsupported,
}
pub(crate) fn emit(
    source: &mut String,
    contract: &CheckedValueContract,
    name: &str,
) -> Result<u16, EmissionRefusal> {
    if conduit_core::primitive_info_kind(contract.value_kind.as_str())
        .is_none_or(|kind| conduit_core::fixed_integer_bytes(kind) == 0)
    {
        return Err(EmissionRefusal::Unsupported);
    }
    contract
        .validate_definition()
        .map_err(|_| EmissionRefusal::Invalid)?;
    let range = match contract.constraints.as_slice() {
        [] => None,
        [ValueConstraint::FixedIntegerRange {
            minimum,
            maximum,
            minimum_endpoint,
            maximum_endpoint,
        }] => Some(FixedIntegerRange {
            minimum: minimum.as_deref(),
            maximum: maximum.as_deref(),
            minimum_endpoint: *minimum_endpoint,
            maximum_endpoint: *maximum_endpoint,
        }),
        _ => return Err(EmissionRefusal::Unsupported),
    };
    let spec = FixedIntegerContractSpec {
        value_kind: contract.value_kind.as_str(),
        maximum_bytes: contract.maximum_bytes,
        range,
    };
    let work = BorrowedFixedIntegerContract::new(&spec)
        .map_err(|_| EmissionRefusal::Invalid)?
        .validation_work();
    let core = "conduit_plot::rust_binding::semantic_core";
    let range = spec.range.map_or_else(|| "None".into(), |range| {
        let bytes = |value: Option<&[u8]>| value.map_or_else(|| "None".into(),
            |bytes| format!("Some(&{bytes:?})"));
        let FixedIntegerRange { minimum, maximum, minimum_endpoint, maximum_endpoint } = range;
        format!("Some({core}::FixedIntegerRange {{ minimum: {}, maximum: {}, minimum_endpoint: {core}::IntervalEndpoint::{minimum_endpoint:?}, maximum_endpoint: {core}::IntervalEndpoint::{maximum_endpoint:?} }})", bytes(minimum), bytes(maximum))
    });
    source.push_str(&format!("static {name}: {core}::FixedIntegerContractSpec<'static> = {core}::FixedIntegerContractSpec {{ value_kind: {:?}, maximum_bytes: {}, range: {range} }};\n", spec.value_kind, spec.maximum_bytes));
    Ok(work)
}
