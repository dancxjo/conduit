use super::{
    StandardConfigurationField, StandardConfigurationRule, StandardKindContract, TerminalBehavior,
    ENABLE_PORT, GATE_KIND, IN_PORT, LATEST_KIND, LEFT_PORT, OUT_PORT, RIGHT_PORT,
    STATE_SELECT_KIND, TEE_KIND,
};
#[cfg(feature = "form-catalog")]
use alloc::string::String;
use alloc::string::ToString;
use alloc::vec;
use alloc::vec::Vec;
use conduit_core::{
    kind_id, port_id, ArtifactId, CapabilityId, CapabilityLimits, CapabilityOffer,
    ConfigurationValue, ExecutionProfileId, HostOperationContractId, HostOperationRequirement,
    ImplementationId, KindContractRevision, PortDescriptor, PortDirection, PortTemporal,
    BOOL_INFO_ID, SCALAR_INFO_ID,
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

pub const FLOW_GATE_SCALAR_CONTRACT_REVISION: &str = "conduit.std/flow-gate-scalar@1";
pub const FLOW_GATE_SCALAR_EXECUTION_PROFILE: &str = "conduit.std/flow-gate-scalar-kernel@1";
pub const FLOW_GATE_SCALAR_IMPLEMENTATION: &str = "std/kernel-flow-gate-scalar@1";
pub const FLOW_GATE_SCALAR_ARTIFACT: &str = "conduit-std-host/flow-gate-scalar@1";
pub const FLOW_GATE_SCALAR_CAPABILITY: &str = "flow-gate-scalar-v1";
pub const FLOW_GATE_BOOL_HOST_OPERATION_CONTRACT: &str = "conduit.host/decode-bool@1";
pub const FLOW_GATE_BOOL_HOST_OPERATION_TARGET: &str = "value/decode-bool";

pub const STATE_SELECT_SCALAR_CONTRACT_REVISION: &str = "conduit.std/state-select-scalar@1";
pub const STATE_SELECT_SCALAR_EXECUTION_PROFILE: &str = "conduit.std/state-select-scalar-kernel@1";
pub const STATE_SELECT_SCALAR_IMPLEMENTATION: &str = "std/kernel-state-select-scalar@1";
pub const STATE_SELECT_SCALAR_ARTIFACT: &str = "conduit-std-host/state-select-scalar@1";
pub const STATE_SELECT_SCALAR_CAPABILITY: &str = "state-select-scalar-v1";

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

pub fn flow_gate_scalar_contract() -> StandardKindContract {
    StandardKindContract {
        kind_id: kind_id(GATE_KIND),
        plain_name: "Gate scalar flow".to_string(),
        summary: "Pass scalar values only while the latest exact Boolean enable is true."
            .to_string(),
        inputs: vec![
            info_port(
                IN_PORT,
                SCALAR_INFO_ID,
                PortDirection::Input,
                PortTemporal::Current,
            ),
            info_port(
                ENABLE_PORT,
                BOOL_INFO_ID,
                PortDirection::Input,
                PortTemporal::Current,
            ),
        ],
        outputs: vec![info_port(
            OUT_PORT,
            SCALAR_INFO_ID,
            PortDirection::Output,
            PortTemporal::Current,
        )],
        configuration: vec![StandardConfigurationField {
            key: "maximum-enable-updates".to_string(),
            default_value: ConfigurationValue::U64(FLOW_STATE_MAXIMUM_VALUES.into()),
            rule: StandardConfigurationRule::U64Range {
                minimum: 1,
                maximum: FLOW_STATE_MAXIMUM_VALUES.into(),
            },
        }],
        limits: limits(),
        terminal_behavior:
            TerminalBehavior::CurrentBooleanGateDefaultsClosedAndCompletesWhenInputsClose,
        hosted_implementation_required: true,
        browser_manifestation_honest: false,
        pico_manifestation_honest: false,
        example: "gate: flow/gate".to_string(),
    }
}

pub fn state_select_scalar_contract() -> StandardKindContract {
    StandardKindContract {
        kind_id: kind_id(STATE_SELECT_KIND),
        plain_name: "Select scalar state".to_string(),
        summary: "Select one of two exact current Scalars using one exact current Boolean."
            .to_string(),
        inputs: vec![
            info_port(
                super::logic::SELECT_SELECTOR_PORT,
                BOOL_INFO_ID,
                PortDirection::Input,
                PortTemporal::Current,
            ),
            info_port(
                super::logic::SELECT_FALSE_PORT,
                SCALAR_INFO_ID,
                PortDirection::Input,
                PortTemporal::Current,
            ),
            info_port(
                super::logic::SELECT_TRUE_PORT,
                SCALAR_INFO_ID,
                PortDirection::Input,
                PortTemporal::Current,
            ),
        ],
        outputs: vec![info_port(
            OUT_PORT,
            SCALAR_INFO_ID,
            PortDirection::Output,
            PortTemporal::Current,
        )],
        configuration: Vec::new(),
        limits: limits(),
        terminal_behavior: TerminalBehavior::CurrentScalarSelectorCompletesWhenInputsClose,
        hosted_implementation_required: true,
        browser_manifestation_honest: false,
        pico_manifestation_honest: false,
        example: "choice: state/select".to_string(),
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

pub fn flow_gate_scalar_offer() -> CapabilityOffer {
    let mut offer = offer(
        flow_gate_scalar_contract(),
        FLOW_GATE_SCALAR_CAPABILITY,
        FLOW_GATE_SCALAR_CONTRACT_REVISION,
        FLOW_GATE_SCALAR_EXECUTION_PROFILE,
        FLOW_GATE_SCALAR_IMPLEMENTATION,
        FLOW_GATE_SCALAR_ARTIFACT,
    );
    offer
        .startup_parameters
        .push(conduit_core::FaceStartupParameter {
            name: "maximum-enable-updates".to_string(),
            value_type: "Count".to_string(),
            has_default: true,
        });
    offer.host_operations.push(HostOperationRequirement {
        contract_id: HostOperationContractId::from(FLOW_GATE_BOOL_HOST_OPERATION_CONTRACT),
        target_kind: Some(kind_id(FLOW_GATE_BOOL_HOST_OPERATION_TARGET)),
        maximum_in_flight: 1,
        maximum_input_bytes: 1,
        maximum_output_bytes: 1,
    });
    offer
}

pub fn state_select_scalar_offer() -> CapabilityOffer {
    offer(
        state_select_scalar_contract(),
        STATE_SELECT_SCALAR_CAPABILITY,
        STATE_SELECT_SCALAR_CONTRACT_REVISION,
        STATE_SELECT_SCALAR_EXECUTION_PROFILE,
        STATE_SELECT_SCALAR_IMPLEMENTATION,
        STATE_SELECT_SCALAR_ARTIFACT,
    )
}

fn port(name: &str, direction: PortDirection, temporal: PortTemporal) -> PortDescriptor {
    info_port(name, SCALAR_INFO_ID, direction, temporal)
}

fn info_port(
    name: &str,
    info: &str,
    direction: PortDirection,
    temporal: PortTemporal,
) -> PortDescriptor {
    PortDescriptor {
        port_id: port_id(name),
        value_kind: kind_id(info),
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
    use conduit_form::{ConfigurationField, ConfigurationRule, KindDefinition, KindSignature};
    for (contract, revision) in [
        (
            state_latest_scalar_contract(),
            STATE_LATEST_SCALAR_CONTRACT_REVISION,
        ),
        (
            flow_tee_scalar_contract(),
            FLOW_TEE_SCALAR_CONTRACT_REVISION,
        ),
        (
            flow_gate_scalar_contract(),
            FLOW_GATE_SCALAR_CONTRACT_REVISION,
        ),
        (
            state_select_scalar_contract(),
            STATE_SELECT_SCALAR_CONTRACT_REVISION,
        ),
    ] {
        startup.insert(KindSignature {
            kind: contract.kind_id.as_str().to_string(),
            startup_parameters: Vec::new(),
        })?;
        let configuration = contract
            .configuration
            .iter()
            .map(|field| ConfigurationField {
                key: field.key.clone(),
                default_value: field.default_value.clone(),
                validation: match field.rule {
                    StandardConfigurationRule::U64Range { minimum, maximum } => {
                        ConfigurationRule::U64Range { minimum, maximum }
                    }
                    _ => unreachable!("flow/state configuration uses only exact integer ranges"),
                },
            })
            .collect();
        profile
            .insert(KindDefinition {
                kind_id: contract.kind_id,
                kind_contract_revision: KindContractRevision::from(revision),
                inputs: contract.inputs,
                outputs: contract.outputs,
                configuration,
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
        let gate = flow_gate_scalar_contract();
        assert_eq!(gate.inputs[0].value_kind.as_str(), SCALAR_INFO_ID);
        assert_eq!(gate.inputs[1].value_kind.as_str(), BOOL_INFO_ID);
        assert_eq!(gate.inputs[1].temporal, PortTemporal::Current);
        assert_eq!(gate.outputs[0].value_kind.as_str(), SCALAR_INFO_ID);
        assert_eq!(gate.configuration.len(), 1);
        assert_eq!(flow_gate_scalar_offer().host_operations.len(), 1);
        let select = state_select_scalar_contract();
        assert!(select
            .inputs
            .iter()
            .chain(select.outputs.iter())
            .all(|port| port.temporal == PortTemporal::Current));
        assert_eq!(select.inputs[0].value_kind.as_str(), BOOL_INFO_ID);
        assert!(select.inputs[1..]
            .iter()
            .chain(select.outputs.iter())
            .all(|port| port.value_kind.as_str() == SCALAR_INFO_ID));

        for port in latest
            .inputs
            .iter()
            .chain(latest.outputs.iter())
            .chain(tee.inputs.iter())
            .chain(tee.outputs.iter())
        {
            assert_eq!(port.value_kind.as_str(), SCALAR_INFO_ID);
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
        let gate = profile.get(&kind_id(GATE_KIND)).unwrap();
        let select = profile.get(&kind_id(STATE_SELECT_KIND)).unwrap();
        assert_eq!(
            latest.kind_contract_revision.as_str(),
            STATE_LATEST_SCALAR_CONTRACT_REVISION
        );
        assert_eq!(
            tee.kind_contract_revision.as_str(),
            FLOW_TEE_SCALAR_CONTRACT_REVISION
        );
        assert_eq!(
            gate.kind_contract_revision.as_str(),
            FLOW_GATE_SCALAR_CONTRACT_REVISION
        );
        assert_eq!(
            select.kind_contract_revision.as_str(),
            STATE_SELECT_SCALAR_CONTRACT_REVISION
        );
    }
}
