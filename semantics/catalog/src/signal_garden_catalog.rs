//! Checked catalog surface for explicit minimal and enriched Garden reducers.

use alloc::{
    string::{String, ToString},
    vec,
    vec::Vec,
};
use conduit_core::{
    kind_id, port_id, KindContractRevision, PortDescriptor, PortDirection, PortTemporal,
    StructuredInfoType,
};
use conduit_form::{KindDefinition, KindSignature};

use crate::{
    garden_clock_observation_type, garden_contact_observation_type, garden_registered_types,
    garden_state_type,
};

pub const GARDEN_FIXTURE_KIND: &str = "garden/deterministic-observations";
pub const GARDEN_MINIMAL_STEP_KIND: &str = "state/garden-step";
pub const GARDEN_ENRICHED_STEP_KIND: &str = "state/garden-step-contact";
pub const GARDEN_CONTRACT_REVISION: &str = "conduit.std/signal-garden-state@1";

pub fn install_signal_garden_catalog(
    startup: &mut conduit_form::StartupCatalog,
    profile: &mut conduit_form::ProfileCatalog,
) -> Result<(), String> {
    for (name, value_type) in garden_registered_types() {
        startup
            .insert_structured_type(name, value_type)
            .map_err(|error| error.to_string())?;
    }
    insert_kind(
        startup,
        profile,
        GARDEN_FIXTURE_KIND,
        vec![],
        vec![
            port("prior", &garden_state_type(), PortDirection::Output),
            port(
                "clock",
                &garden_clock_observation_type(),
                PortDirection::Output,
            ),
            port(
                "contact",
                &garden_contact_observation_type(),
                PortDirection::Output,
            ),
        ],
    )?;
    insert_kind(
        startup,
        profile,
        GARDEN_MINIMAL_STEP_KIND,
        reducer_inputs(false),
        vec![port("next", &garden_state_type(), PortDirection::Output)],
    )?;
    insert_kind(
        startup,
        profile,
        GARDEN_ENRICHED_STEP_KIND,
        reducer_inputs(true),
        vec![port("next", &garden_state_type(), PortDirection::Output)],
    )
}

fn reducer_inputs(enriched: bool) -> Vec<PortDescriptor> {
    let mut inputs = vec![
        port("prior", &garden_state_type(), PortDirection::Input),
        port(
            "clock",
            &garden_clock_observation_type(),
            PortDirection::Input,
        ),
    ];
    if enriched {
        inputs.push(port(
            "contact",
            &garden_contact_observation_type(),
            PortDirection::Input,
        ));
    }
    inputs
}

fn insert_kind(
    startup: &mut conduit_form::StartupCatalog,
    profile: &mut conduit_form::ProfileCatalog,
    kind: &str,
    inputs: Vec<PortDescriptor>,
    outputs: Vec<PortDescriptor>,
) -> Result<(), String> {
    startup.insert(KindSignature {
        kind: kind.into(),
        startup_parameters: vec![],
    })?;
    profile
        .insert(KindDefinition {
            kind_id: kind_id(kind),
            kind_contract_revision: KindContractRevision::from(GARDEN_CONTRACT_REVISION),
            inputs,
            outputs,
            configuration: vec![],
        })
        .map_err(|error| error.to_string())
}

fn port(name: &str, value_type: &StructuredInfoType, direction: PortDirection) -> PortDescriptor {
    PortDescriptor {
        port_id: port_id(name),
        value_kind: value_type
            .profile()
            .expect("reviewed Garden profile")
            .value_kind()
            .clone(),
        direction,
        temporal: PortTemporal::Value,
    }
}
