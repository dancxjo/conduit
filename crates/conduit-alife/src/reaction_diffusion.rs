//! Portable Form contract for bounded reaction-diffusion evolution.

use alloc::{string::ToString, vec, vec::Vec};
use conduit_core::{
    kind_id, port_id, KindContractRevision, PortDescriptor, PortDirection, PortTemporal,
    REACTION_DIFFUSION_REQUEST_INFO_ID, REACTION_DIFFUSION_STATE_INFO_ID,
};
use conduit_form::{KindDefinition, KindSignature, ProfileCatalog, StartupCatalog};

pub const REACTION_DIFFUSION_EVOLVE_KIND: &str = "field/evolve";
pub const REACTION_DIFFUSION_KIND_REVISION: &str = "conduit.std/field-evolve@1";

pub fn install_reaction_diffusion_catalogs(
    startup: &mut StartupCatalog,
    profile: &mut ProfileCatalog,
) -> Result<(), alloc::string::String> {
    let definition = reaction_diffusion_definition();
    startup.insert(KindSignature {
        kind: REACTION_DIFFUSION_EVOLVE_KIND.into(),
        startup_parameters: vec![],
    })?;
    profile
        .insert(definition)
        .map_err(|error| error.to_string())
}

pub fn reaction_diffusion_definition() -> KindDefinition {
    KindDefinition {
        kind_id: kind_id(REACTION_DIFFUSION_EVOLVE_KIND),
        kind_contract_revision: KindContractRevision::from(REACTION_DIFFUSION_KIND_REVISION),
        inputs: reaction_diffusion_inputs(),
        outputs: reaction_diffusion_outputs(),
        configuration: vec![],
    }
}

pub fn reaction_diffusion_inputs() -> Vec<PortDescriptor> {
    vec![
        value_port(
            "state",
            REACTION_DIFFUSION_STATE_INFO_ID,
            PortDirection::Input,
        ),
        value_port(
            "request",
            REACTION_DIFFUSION_REQUEST_INFO_ID,
            PortDirection::Input,
        ),
    ]
}

pub fn reaction_diffusion_outputs() -> Vec<PortDescriptor> {
    vec![value_port(
        "next-state",
        REACTION_DIFFUSION_STATE_INFO_ID,
        PortDirection::Output,
    )]
}

fn value_port(name: &str, value_kind: &str, direction: PortDirection) -> PortDescriptor {
    PortDescriptor {
        port_id: port_id(name),
        value_kind: kind_id(value_kind),
        direction,
        temporal: PortTemporal::Value,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn portable_contract_has_exact_direction_distinct_ports() {
        let definition = reaction_diffusion_definition();
        assert_eq!(definition.kind_id.as_str(), REACTION_DIFFUSION_EVOLVE_KIND);
        assert_eq!(definition.inputs[0].port_id.as_str(), "state");
        assert_eq!(definition.inputs[1].port_id.as_str(), "request");
        assert_eq!(definition.outputs[0].port_id.as_str(), "next-state");
        assert_eq!(
            definition.inputs[0].value_kind,
            definition.outputs[0].value_kind
        );
        assert_eq!(definition.inputs[0].direction, PortDirection::Input);
        assert_eq!(definition.outputs[0].direction, PortDirection::Output);
    }
}
