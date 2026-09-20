//! Portable structured-robotics Form catalog.

use alloc::{
    string::{String, ToString},
    vec,
    vec::Vec,
};
use conduit_core::{kind_id, KindIdentity, PortDescriptor};
use conduit_form::{KindDefinition, KindSignature};

use conduit_robotics::{
    robotics_structured_kind_contracts, robotics_structured_registered_types,
    ROBOTICS_STRUCTURED_REVISION,
};

pub fn install_robotics_structured_catalogs(
    startup: &mut conduit_form::StartupCatalog,
    profile: &mut conduit_form::ProfileCatalog,
) -> Result<(), String> {
    for (name, value_type) in robotics_structured_registered_types() {
        startup
            .insert_structured_type(name, value_type)
            .map_err(|error| error.to_string())?;
    }
    for (kind, inputs, outputs) in robotics_structured_kind_contracts() {
        insert_kind(startup, profile, kind.as_str(), inputs, outputs)?;
    }
    Ok(())
}

fn insert_kind(
    startup: &mut conduit_form::StartupCatalog,
    profile: &mut conduit_form::ProfileCatalog,
    kind: &str,
    inputs: Vec<PortDescriptor>,
    outputs: Vec<PortDescriptor>,
) -> Result<(), String> {
    startup
        .insert(KindSignature {
            kind: kind.into(),
            startup_parameters: vec![],
        })
        .map_err(|error| error.to_string())?;
    profile
        .insert(KindDefinition {
            kind_id: kind_id(kind),
            kind_contract_revision: KindIdentity::from(ROBOTICS_STRUCTURED_REVISION),
            inputs,
            outputs,
            configuration: vec![],
        })
        .map_err(|error| error.to_string())
}
