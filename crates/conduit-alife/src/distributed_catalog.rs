//! Realization-only Kinds for one reviewed three-region Lenia Back.

use alloc::{string::ToString, vec, vec::Vec};
use conduit_core::{
    kind_id, port_id, KindContractRevision, PortDescriptor, PortDirection, PortTemporal,
    SCALAR_FIELD2_INFO_ID,
};
use conduit_form::{KindDefinition, KindSignature, ProfileCatalog, StartupCatalog};

pub const LENIA_PARTITION_KIND: &str = "alife/lenia-partition-three";
pub const LENIA_REGION_STEP_KIND: &str = "alife/lenia-region-step";
pub const LENIA_JOIN_KIND: &str = "alife/lenia-join-three";
pub const LENIA_REGION_WORK_INFO_ID: &str = "alife/lenia-region-work@1";
pub const LENIA_REGION_RESULT_INFO_ID: &str = "alife/lenia-region-result@1";
pub const SCALAR_FIELD_GRAY8_KIND: &str = "graphics/scalar-field-gray8";

pub fn install_distributed_lenia_catalogs(
    startup: &mut StartupCatalog,
    profile: &mut ProfileCatalog,
) -> Result<(), alloc::string::String> {
    for definition in distributed_definitions() {
        startup.insert(KindSignature {
            kind: definition.kind_id.as_str().to_string(),
            startup_parameters: vec![],
        })?;
        profile
            .insert(definition)
            .map_err(|error| error.to_string())?;
    }
    Ok(())
}

pub fn distributed_definitions() -> Vec<KindDefinition> {
    vec![
        partition_definition(),
        worker_definition(),
        join_definition(),
        field_bitmap_definition(),
    ]
}

fn field_bitmap_definition() -> KindDefinition {
    KindDefinition {
        kind_id: kind_id(SCALAR_FIELD_GRAY8_KIND),
        kind_contract_revision: KindContractRevision::from("conduit.graphics/scalar-field-gray8@1"),
        inputs: vec![port(
            "field",
            SCALAR_FIELD2_INFO_ID,
            PortDirection::Input,
            closing_flow(),
        )],
        outputs: vec![port(
            "bitmap",
            conduit_presentation::GRAY8_BITMAP_INFO_KIND,
            PortDirection::Output,
            closing_flow(),
        )],
        configuration: vec![],
    }
}

fn partition_definition() -> KindDefinition {
    KindDefinition {
        kind_id: kind_id(LENIA_PARTITION_KIND),
        kind_contract_revision: KindContractRevision::from("conduit.alife/lenia-partition-three@1"),
        inputs: vec![
            port(
                "initial",
                SCALAR_FIELD2_INFO_ID,
                PortDirection::Input,
                PortTemporal::Value,
            ),
            port(
                "tick",
                conduit_std_catalog::TICK_VALUE_KIND,
                PortDirection::Input,
                closing_flow(),
            ),
        ],
        outputs: (0..3)
            .map(|index| {
                port(
                    &alloc::format!("work{index}"),
                    LENIA_REGION_WORK_INFO_ID,
                    PortDirection::Output,
                    closing_flow(),
                )
            })
            .collect(),
        configuration: vec![],
    }
}

fn worker_definition() -> KindDefinition {
    KindDefinition {
        kind_id: kind_id(LENIA_REGION_STEP_KIND),
        kind_contract_revision: KindContractRevision::from("conduit.alife/lenia-region-step@1"),
        inputs: vec![port(
            "work",
            LENIA_REGION_WORK_INFO_ID,
            PortDirection::Input,
            closing_flow(),
        )],
        outputs: vec![port(
            "result",
            LENIA_REGION_RESULT_INFO_ID,
            PortDirection::Output,
            closing_flow(),
        )],
        configuration: vec![],
    }
}

fn join_definition() -> KindDefinition {
    KindDefinition {
        kind_id: kind_id(LENIA_JOIN_KIND),
        kind_contract_revision: KindContractRevision::from("conduit.alife/lenia-join-three@1"),
        inputs: (0..3)
            .map(|index| {
                port(
                    &alloc::format!("result{index}"),
                    LENIA_REGION_RESULT_INFO_ID,
                    PortDirection::Input,
                    closing_flow(),
                )
            })
            .collect(),
        outputs: vec![port(
            "field",
            SCALAR_FIELD2_INFO_ID,
            PortDirection::Output,
            closing_flow(),
        )],
        configuration: vec![],
    }
}

fn port(
    name: &str,
    value_kind: &str,
    direction: PortDirection,
    temporal: PortTemporal,
) -> PortDescriptor {
    PortDescriptor {
        port_id: port_id(name),
        value_kind: kind_id(value_kind),
        direction,
        temporal,
    }
}

const fn closing_flow() -> PortTemporal {
    PortTemporal::Flow { closes: true }
}
