//! Ordinary Form-facing tensor port contracts.

use alloc::{
    string::{String, ToString},
    vec,
};
use conduit_core::{
    kind_id, port_id, KindContractRevision, PortDescriptor, PortDirection, PortTemporal,
};
use conduit_form::{KindDefinition, KindSignature, ProfileCatalog, StartupCatalog};

use crate::TENSOR_INFO_ID;

pub const TENSOR_FIXTURE_KIND: &str = "data/tensor-fixture";
pub const TENSOR_IDENTITY_KIND: &str = "data/tensor-identity";
pub const TENSOR_CONTRACT_REVISION: &str = "conduit.std/tensor@1";

pub fn install_tensor_catalogs(
    startup: &mut StartupCatalog,
    profile: &mut ProfileCatalog,
) -> Result<(), String> {
    for kind in [TENSOR_FIXTURE_KIND, TENSOR_IDENTITY_KIND] {
        startup
            .insert(KindSignature {
                kind: kind.into(),
                startup_parameters: vec![],
            })
            .map_err(|error| error.to_string())?;
    }
    profile
        .insert(KindDefinition {
            kind_id: kind_id(TENSOR_FIXTURE_KIND),
            kind_contract_revision: KindContractRevision::from(TENSOR_CONTRACT_REVISION),
            inputs: vec![],
            outputs: vec![port("tensor", PortDirection::Output)],
            configuration: vec![],
        })
        .map_err(|error| error.to_string())?;
    profile
        .insert(KindDefinition {
            kind_id: kind_id(TENSOR_IDENTITY_KIND),
            kind_contract_revision: KindContractRevision::from(TENSOR_CONTRACT_REVISION),
            inputs: vec![port("tensor", PortDirection::Input)],
            outputs: vec![port("tensor", PortDirection::Output)],
            configuration: vec![],
        })
        .map_err(|error| error.to_string())
}

fn port(name: &str, direction: PortDirection) -> PortDescriptor {
    PortDescriptor {
        port_id: port_id(name),
        value_kind: kind_id(TENSOR_INFO_ID),
        direction,
        temporal: PortTemporal::Value,
    }
}
