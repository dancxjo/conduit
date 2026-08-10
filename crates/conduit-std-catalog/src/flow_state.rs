use super::{
    StandardKindContract, TerminalBehavior, IN_PORT, LATEST_KIND, LEFT_PORT, OUT_PORT, RIGHT_PORT,
    TEE_KIND,
};
#[cfg(feature = "form-catalog")]
use alloc::string::String;
use alloc::string::ToString;
use alloc::vec;
use alloc::vec::Vec;
use conduit_core::{
    kind_id, port_id, ArtifactId, CapabilityId, CapabilityLimits, CapabilityOffer,
    ExecutionProfileId, ImplementationId, KindContractRevision, PortDescriptor, PortDirection,
    PortTemporal, SCALAR_INFO_ID,
};

pub const STATE_LATEST_SCALAR_CONTRACT_REVISION: &str = "conduit.std/state-latest-scalar@2";
pub const STATE_LATEST_SCALAR_EXECUTION_PROFILE: &str = "conduit.std/state-latest-scalar-kernel@2";
pub const STATE_LATEST_SCALAR_IMPLEMENTATION: &str = "std/kernel-state-latest-scalar@2";
pub const STATE_LATEST_SCALAR_ARTIFACT: &str = "conduit-std-host/state-latest-scalar@2";
pub const STATE_LATEST_SCALAR_CAPABILITY: &str = "state-latest-scalar-v2";

pub const FLOW_TEE_SCALAR_CONTRACT_REVISION: &str = "conduit.std/flow-tee-scalar@2";
pub const FLOW_TEE_SCALAR_EXECUTION_PROFILE: &str = "conduit.std/flow-tee-scalar-kernel@2";
pub const FLOW_TEE_SCALAR_IMPLEMENTATION: &str = "std/kernel-flow-tee-scalar@2";
pub const FLOW_TEE_SCALAR_ARTIFACT: &str = "conduit-std-host/flow-tee-scalar@2";
pub const FLOW_TEE_SCALAR_CAPABILITY: &str = "flow-tee-scalar-v2";

pub const FLOW_STATE_MAXIMUM_VALUES: u16 = 16;

pub fn state_latest_scalar_contract() -> StandardKindContract {
    StandardKindContract {
        kind_id: kind_id(LATEST_KIND),
        plain_name: "Latest scalar state".to_string(),
        summary: "Retain at most one scalar and emit each replacement as current state."
            .to_string(),
        inputs: vec![port(
            IN_PORT,
            PortDirection::Input,
            PortTemporal::Flow { closes: true },
        )],
        outputs: vec![port(OUT_PORT, PortDirection::Output, PortTemporal::Current)],
        configuration: Vec::new(),
        limits: limits(),
        terminal_behavior: TerminalBehavior::EmitsCurrentAndCompletesWhenInputCloses,
        hosted_implementation_required: true,
        browser_manifestation_honest: false,
        pico_manifestation_honest: false,
        example: "latest: state/latest".to_string(),
    }
}

pub fn flow_tee_scalar_contract() -> StandardKindContract {
    StandardKindContract {
        kind_id: kind_id(TEE_KIND),
        plain_name: "Tee scalar state".to_string(),
        summary: "Deliver each scalar current atomically to two coupled output branches."
            .to_string(),
        inputs: vec![port(IN_PORT, PortDirection::Input, PortTemporal::Current)],
        outputs: vec![
            port(LEFT_PORT, PortDirection::Output, PortTemporal::Current),
            port(RIGHT_PORT, PortDirection::Output, PortTemporal::Current),
        ],
        configuration: Vec::new(),
        limits: limits(),
        terminal_behavior: TerminalBehavior::CoupledAtomicFanoutAndMirrorsInputTerminal,
        hosted_implementation_required: true,
        browser_manifestation_honest: false,
        pico_manifestation_honest: false,
        example: "split: flow/tee".to_string(),
    }
}

pub fn state_latest_scalar_offer() -> CapabilityOffer {
    offer(
        state_latest_scalar_contract(),
        STATE_LATEST_SCALAR_CAPABILITY,
        STATE_LATEST_SCALAR_CONTRACT_REVISION,
        STATE_LATEST_SCALAR_EXECUTION_PROFILE,
        STATE_LATEST_SCALAR_IMPLEMENTATION,
        STATE_LATEST_SCALAR_ARTIFACT,
    )
}

pub fn flow_tee_scalar_offer() -> CapabilityOffer {
    offer(
        flow_tee_scalar_contract(),
        FLOW_TEE_SCALAR_CAPABILITY,
        FLOW_TEE_SCALAR_CONTRACT_REVISION,
        FLOW_TEE_SCALAR_EXECUTION_PROFILE,
        FLOW_TEE_SCALAR_IMPLEMENTATION,
        FLOW_TEE_SCALAR_ARTIFACT,
    )
}

fn port(name: &str, direction: PortDirection, temporal: PortTemporal) -> PortDescriptor {
    PortDescriptor {
        port_id: port_id(name),
        value_kind: kind_id(SCALAR_INFO_ID),
        direction,
        temporal,
    }
}

fn limits() -> CapabilityLimits {
    CapabilityLimits {
        max_active_instances: 16,
        max_queue_items: 4,
        max_queue_bytes: 32,
    }
}

fn offer(
    contract: StandardKindContract,
    capability: &str,
    revision: &str,
    profile: &str,
    implementation: &str,
    artifact: &str,
) -> CapabilityOffer {
    CapabilityOffer {
        startup_parameters: Vec::new(),
        shorthand: None,
        capability_id: CapabilityId::from(capability),
        kind_id: contract.kind_id,
        kind_contract_revision: KindContractRevision::from(revision),
        implementation: conduit_core::ImplementationOffer {
            execution_profile_id: ExecutionProfileId::from(profile),
            implementation_id: ImplementationId::from(implementation),
            artifact_id: ArtifactId::from(artifact),
        },
        inputs: contract.inputs,
        outputs: contract.outputs,
        host_operations: Vec::new(),
        resource_requirements: Vec::new(),
        authority_requirements: Vec::new(),
        limits: contract.limits,
    }
}

#[cfg(feature = "form-catalog")]
pub fn install_flow_state_catalogs(
    startup: &mut conduit_form::StartupCatalog,
    profile: &mut conduit_form::ProfileCatalog,
) -> Result<(), String> {
    use conduit_form::{KindDefinition, KindSignature};
    for (contract, revision) in [
        (
            state_latest_scalar_contract(),
            STATE_LATEST_SCALAR_CONTRACT_REVISION,
        ),
        (
            flow_tee_scalar_contract(),
            FLOW_TEE_SCALAR_CONTRACT_REVISION,
        ),
    ] {
        startup.insert(KindSignature {
            kind: contract.kind_id.as_str().to_string(),
            startup_parameters: Vec::new(),
        })?;
        profile
            .insert(KindDefinition {
                kind_id: contract.kind_id,
                kind_contract_revision: KindContractRevision::from(revision),
                inputs: contract.inputs,
                outputs: contract.outputs,
                configuration: Vec::new(),
            })
            .map_err(|error| error.to_string())?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn contracts_are_exact_scalar_state_without_legacy_any() {
        let latest = state_latest_scalar_contract();
        assert_eq!(
            latest.inputs[0].temporal,
            PortTemporal::Flow { closes: true }
        );
        assert_eq!(latest.outputs[0].temporal, PortTemporal::Current);

        let tee = flow_tee_scalar_contract();
        assert_eq!(tee.inputs[0].temporal, PortTemporal::Current);
        assert_eq!(tee.outputs.len(), 2);
        for port in latest
            .inputs
            .iter()
            .chain(latest.outputs.iter())
            .chain(tee.inputs.iter())
            .chain(tee.outputs.iter())
        {
            assert_eq!(port.value_kind.as_str(), SCALAR_INFO_ID);
            assert_ne!(port.value_kind.as_str(), super::super::GENERIC_VALUE_KIND);
        }
    }

    #[cfg(feature = "form-catalog")]
    #[test]
    fn exact_revisions_install_without_executable_type_wrappers() {
        let mut startup = conduit_form::StartupCatalog::new();
        let mut profile = conduit_form::ProfileCatalog::new();
        install_flow_state_catalogs(&mut startup, &mut profile).unwrap();
        let latest = profile.get(&kind_id(LATEST_KIND)).unwrap();
        let tee = profile.get(&kind_id(TEE_KIND)).unwrap();
        assert_eq!(
            latest.kind_contract_revision.as_str(),
            STATE_LATEST_SCALAR_CONTRACT_REVISION
        );
        assert_eq!(
            tee.kind_contract_revision.as_str(),
            FLOW_TEE_SCALAR_CONTRACT_REVISION
        );
    }
}
