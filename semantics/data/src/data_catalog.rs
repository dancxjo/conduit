//! Exact form-facing contracts for explicit Text publication and loading.

use alloc::{string::ToString, vec};
use conduit_core::{
    data_reference_kind, kind_id, port_id, CapabilityLimits, CheckedValueContract,
    FrontValueContract, FrontValueLocation, Kind, KindIdentity, KindSemanticLaw, PortDescriptor,
    PortDirection, PortTemporal,
};
use conduit_form::{KindProjection, KindSignature, ProfileCatalog, StartupCatalog};

pub const DATA_SAVE_TEXT_KIND: &str = "data/save/text";
pub const DATA_LOAD_TEXT_KIND: &str = "data/load/text";
use crate::{
    maximum_data_reference_encoded_bytes, DATA_LOAD_TEXT_TERMINAL_INFO_ID,
    DATA_SAVE_TEXT_TERMINAL_INFO_ID, DATA_TEXT_TERMINAL_ENCODED_LEN,
};

pub const DATA_TEXT_CONTRACT_REVISION: &str = "conduit.data/text-generation@2";
pub const MAXIMUM_DATA_TEXT_BYTES: u32 = 4 * 1024;
pub const MAXIMUM_DATA_TEXT_ACTIVE_INSTANCES: u16 = 16;

pub fn install_data_text_catalogs(
    startup: &mut StartupCatalog,
    profile: &mut ProfileCatalog,
) -> Result<(), alloc::string::String> {
    for kind in [DATA_SAVE_TEXT_KIND, DATA_LOAD_TEXT_KIND] {
        startup.insert(KindSignature {
            kind: kind.to_string(),
            startup_parameters: vec![],
        })?;
    }
    profile
        .insert_kind(data_save_text_contract())
        .map_err(|error| error.to_string())?;
    profile
        .insert_kind(data_load_text_contract())
        .map_err(|error| error.to_string())?;
    Ok(())
}

pub fn data_save_text_projection() -> KindProjection {
    let text = kind_id("value/text");
    KindProjection {
        kind_id: kind_id(DATA_SAVE_TEXT_KIND),
        kind_contract_revision: KindIdentity::from(DATA_TEXT_CONTRACT_REVISION),
        inputs: vec![port("value", text.clone(), PortDirection::Input)],
        outputs: vec![terminal_port(
            "data",
            data_reference_kind(&text),
            PortDirection::Output,
            DATA_SAVE_TEXT_TERMINAL_INFO_ID,
        )],
        configuration: vec![],
    }
}

pub fn data_load_text_projection() -> KindProjection {
    let text = kind_id("value/text");
    KindProjection {
        kind_id: kind_id(DATA_LOAD_TEXT_KIND),
        kind_contract_revision: KindIdentity::from(DATA_TEXT_CONTRACT_REVISION),
        inputs: vec![port(
            "data",
            data_reference_kind(&text),
            PortDirection::Input,
        )],
        outputs: vec![terminal_port(
            "value",
            text,
            PortDirection::Output,
            DATA_LOAD_TEXT_TERMINAL_INFO_ID,
        )],
        configuration: vec![],
    }
}

pub fn data_save_text_contract() -> Kind {
    contract(data_save_text_projection())
}

pub fn data_load_text_contract() -> Kind {
    contract(data_load_text_projection())
}

fn contract(projection: KindProjection) -> Kind {
    let text = kind_id("value/text");
    let reference = data_reference_kind(&text);
    let reference_bytes = maximum_data_reference_encoded_bytes(text.as_str())
        .expect("canonical Text identity has an exact reference envelope")
        as u32;
    let value_contract = |location, value_kind, maximum_bytes| FrontValueContract {
        location,
        contract: CheckedValueContract::new(value_kind, maximum_bytes, vec![])
            .expect("data Text Fore value envelope is finite"),
    };
    let value_contracts = match projection.kind_id.as_str() {
        DATA_SAVE_TEXT_KIND => vec![
            value_contract(
                FrontValueLocation::Input(port_id("value")),
                text,
                MAXIMUM_DATA_TEXT_BYTES,
            ),
            value_contract(
                FrontValueLocation::Output(port_id("data")),
                reference,
                reference_bytes,
            ),
            value_contract(
                FrontValueLocation::OutputAbnormal(port_id("data")),
                kind_id(DATA_SAVE_TEXT_TERMINAL_INFO_ID),
                DATA_TEXT_TERMINAL_ENCODED_LEN as u32,
            ),
        ],
        DATA_LOAD_TEXT_KIND => vec![
            value_contract(
                FrontValueLocation::Input(port_id("data")),
                reference,
                reference_bytes,
            ),
            value_contract(
                FrontValueLocation::Output(port_id("value")),
                text,
                MAXIMUM_DATA_TEXT_BYTES,
            ),
            value_contract(
                FrontValueLocation::OutputAbnormal(port_id("value")),
                kind_id(DATA_LOAD_TEXT_TERMINAL_INFO_ID),
                DATA_TEXT_TERMINAL_ENCODED_LEN as u32,
            ),
        ],
        _ => unreachable!("data Text contract only accepts reviewed projections"),
    };
    Kind {
        startup_parameters: vec![],
        shorthand: None,
        kind_id: projection.kind_id,
        kind_contract_revision: projection.kind_contract_revision,
        inputs: projection.inputs,
        outputs: projection.outputs,
        configuration: projection.configuration,
        semantic_laws: vec![KindSemanticLaw::ValueContracts(value_contracts)],
        limits: CapabilityLimits {
            max_active_instances: MAXIMUM_DATA_TEXT_ACTIVE_INSTANCES,
            max_queue_items: 1,
            max_queue_bytes: MAXIMUM_DATA_TEXT_BYTES,
        },
    }
}

fn terminal_port(
    name: &str,
    value_kind: conduit_core::KindId,
    direction: PortDirection,
    abnormal_kind: &str,
) -> PortDescriptor {
    PortDescriptor {
        port_id: port_id(name),
        value_kind,
        direction,
        temporal: PortTemporal::Value,
        abnormal_kind: Some(kind_id(abnormal_kind)),
    }
}

fn port(name: &str, value_kind: conduit_core::KindId, direction: PortDirection) -> PortDescriptor {
    PortDescriptor {
        port_id: port_id(name),
        value_kind,
        direction,
        temporal: PortTemporal::Value,
        abnormal_kind: None,
    }
}
