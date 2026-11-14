//! Emit supported checked predicates as immutable data, never runtime decoders.
use super::*;
use conduit_core::CheckedValueContract;
pub(crate) fn emit(
    source: &mut String,
    contract: Option<&CheckedValueContract>,
    name: &str,
    kind: PrimitiveInfoKind,
) -> Result<String, Refusal> {
    emit_with_work(source, contract, name, kind).map(|(source, _)| source)
}
pub(crate) fn emit_with_work(
    source: &mut String,
    contract: Option<&CheckedValueContract>,
    name: &str,
    kind: PrimitiveInfoKind,
) -> Result<(String, u16), Refusal> {
    let Some(contract) = contract else {
        return Ok(("None".into(), 0));
    };
    let unsupported =
        || Refusal::UnsupportedType("scalar-image/value-contract-witness-unavailable@1".into());
    if conduit_core::fixed_integer_bytes(kind) == 0 {
        return Err(unsupported());
    }
    if conduit_core::primitive_info_kind(contract.value_kind.as_str()) != Some(kind) {
        return Err(Refusal::InvalidProgram);
    }
    let work =
        crate::rust_binding::resident_contract::emit(source, contract, name).map_err(|reason| {
            match reason {
                crate::rust_binding::resident_contract::EmissionRefusal::Invalid => {
                    Refusal::InvalidProgram
                }
                crate::rust_binding::resident_contract::EmissionRefusal::Unsupported => {
                    unsupported()
                }
            }
        })?;
    Ok((format!("Some(&{name})"), work))
}
