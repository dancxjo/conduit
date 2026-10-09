//! Exact compiled Type contracts; no Source, Host or authority acceptance.
extern crate alloc;
use alloc::{format, string::String, vec::Vec};
use conduit_core::{CheckedValueContract, StructuredInfoType};
use conduit_plot::{CheckedNativeType, NativeTypeValueContract, PortableExpressionProgram};

pub type CompiledType = (
    String,
    String,
    Vec<u8>,
    Vec<(String, CheckedValueContract)>,
    Vec<Vec<u8>>,
);

pub fn decode(bytes: &[u8], source_id: &str) -> Result<Vec<CheckedNativeType>, String> {
    let ((retained_source, types), remaining): ((String, Vec<CompiledType>), &[u8]) =
        postcard::take_from_bytes(bytes)
            .map_err(|error| format!("compiled speech Types: {error}"))?;
    if !remaining.is_empty() {
        return Err("compiled speech Types have trailing bytes".into());
    }
    if retained_source != source_id {
        return Err("compiled speech Types belong to another Source".into());
    }
    types
        .into_iter()
        .map(|(name, identity, value_type, contracts, invariants)| {
            Ok(CheckedNativeType {
                name,
                identity: conduit_core::kind_id(&identity),
                value_type: StructuredInfoType::from_canonical_bytes(&value_type)
                    .map_err(|error| format!("compiled speech Type: {error:?}"))?,
                value_contracts: contracts
                    .into_iter()
                    .map(|(representation_path, contract)| {
                        Ok(NativeTypeValueContract {
                            representation_path,
                            contract: CheckedValueContract::new(
                                contract.value_kind,
                                contract.maximum_bytes,
                                contract.constraints,
                            )
                            .map_err(|error| format!("compiled speech contract: {error:?}"))?,
                        })
                    })
                    .collect::<Result<_, String>>()?,
                invariants: invariants
                    .into_iter()
                    .map(|program| {
                        PortableExpressionProgram::from_canonical_bytes(&program)
                            .map_err(|error| format!("compiled speech law: {error:?}"))
                    })
                    .collect::<Result<_, String>>()?,
            })
        })
        .collect()
}
