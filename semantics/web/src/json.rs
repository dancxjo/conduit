//! Portable bounded JSON encode/decode Kind meaning.

use crate::PortableKindContract;
#[cfg(feature = "form-catalog")]
use alloc::string::ToString;
use alloc::vec;
use conduit_core::{
    kind_id, port_id, CapabilityLimits, KindContractRevision, PortDescriptor, PortDirection,
    PortTemporal,
};

pub const JSON_ENCODE_KIND: &str = "json/encode";
pub const JSON_DECODE_KIND: &str = "json/decode";
pub const JSON_ENCODE_REVISION: &str = "conduit.std/json-encode@1";
pub const JSON_DECODE_REVISION: &str = "conduit.std/json-decode@1";
pub const JSON_COLLECTION_STEP_KIND: &str = "json/collection-step";
pub const JSON_COLLECTION_STEP_REVISION: &str = "conduit.json/collection-step@1";

pub fn json_collection_step_semantics() -> PortableKindContract {
    contract(
        JSON_COLLECTION_STEP_KIND,
        JSON_COLLECTION_STEP_REVISION,
        crate::JSON_INFO_ID,
        crate::JSON_INFO_ID,
    )
}

pub fn json_encode_semantics() -> PortableKindContract {
    contract(
        JSON_ENCODE_KIND,
        JSON_ENCODE_REVISION,
        crate::JSON_INFO_ID,
        crate::JSON_TEXT_INFO_ID,
    )
}

pub fn json_decode_semantics() -> PortableKindContract {
    contract(
        JSON_DECODE_KIND,
        JSON_DECODE_REVISION,
        crate::JSON_TEXT_INFO_ID,
        crate::JSON_INFO_ID,
    )
}

#[cfg(feature = "form-catalog")]
pub fn install_json_catalogs(
    startup: &mut conduit_form::StartupCatalog,
    profile: &mut conduit_form::ProfileCatalog,
) -> Result<(), alloc::string::String> {
    use alloc::vec::Vec;
    use conduit_form::{KindDefinition, KindSignature};
    for contract in [
        json_encode_semantics(),
        json_decode_semantics(),
        json_collection_step_semantics(),
    ] {
        startup.insert(KindSignature {
            kind: contract.kind_id.as_str().to_string(),
            startup_parameters: Vec::new(),
        })?;
        profile
            .insert(KindDefinition {
                kind_id: contract.kind_id,
                kind_contract_revision: contract.kind_contract_revision,
                inputs: contract.inputs,
                outputs: contract.outputs,
                configuration: Vec::new(),
            })
            .map_err(|error| error.to_string())?;
    }
    crate::install_json_boolean_summary_catalog(startup, profile)
}

fn contract(kind: &str, revision: &str, input: &str, output: &str) -> PortableKindContract {
    PortableKindContract {
        kind_id: kind_id(kind),
        kind_contract_revision: KindContractRevision::from(revision),
        inputs: vec![port(input, PortDirection::Input)],
        outputs: vec![port(output, PortDirection::Output)],
        limits: CapabilityLimits {
            max_active_instances: 8,
            max_queue_items: 4,
            max_queue_bytes: crate::JSON_MAXIMUM_ENCODED_BYTES as u32,
        },
    }
}

fn port(value: &str, direction: PortDirection) -> PortDescriptor {
    PortDescriptor {
        port_id: port_id("value"),
        value_kind: kind_id(value),
        direction,
        temporal: PortTemporal::Value,
    }
}
