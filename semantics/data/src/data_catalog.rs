//! Exact form-facing contracts for explicit Text publication and loading.

use alloc::{string::ToString, vec};
use conduit_core::{
    data_reference_kind, kind_id, port_id, CapabilityLimits, Kind, KindIdentity, PortDescriptor,
    PortDirection, PortTemporal,
};
use conduit_form::{KindProjection, KindSignature, ProfileCatalog, StartupCatalog};

pub const DATA_SAVE_TEXT_KIND: &str = "data/save/text";
pub const DATA_LOAD_TEXT_KIND: &str = "data/load/text";
pub const DATA_TEXT_CONTRACT_REVISION: &str = "conduit.data/text-generation@1";
pub const MAXIMUM_DATA_TEXT_BYTES: u32 = 4 * 1024;

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
        outputs: vec![port(
            "data",
            data_reference_kind(&text),
            PortDirection::Output,
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
        outputs: vec![port("value", text, PortDirection::Output)],
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
    Kind {
        startup_parameters: vec![],
        shorthand: None,
        kind_id: projection.kind_id,
        kind_contract_revision: projection.kind_contract_revision,
        inputs: projection.inputs,
        outputs: projection.outputs,
        configuration: projection.configuration,
        semantic_laws: Default::default(),
        limits: CapabilityLimits {
            max_active_instances: 1,
            max_queue_items: 1,
            max_queue_bytes: MAXIMUM_DATA_TEXT_BYTES,
        },
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
