//! Emit supported checked predicates as immutable data, never runtime decoders.
use super::*;
use conduit_core::CheckedValueContract;
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
    crate::rust_binding::resident_contract::emit(source, contract, name).map_err(|reason| {
        match reason {
            crate::rust_binding::resident_contract::EmissionRefusal::Invalid => {
                Refusal::InvalidProgram
            }
            crate::rust_binding::resident_contract::EmissionRefusal::Unsupported => unsupported(),
        }
    })?;
    Ok(format!("Some(&{name})"))
}
