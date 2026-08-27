use super::{
    StandardConfigurationField, StandardConfigurationRule, StandardKindContract, TerminalBehavior,
};
use alloc::string::{String, ToString};
use alloc::vec;
use alloc::vec::Vec;
use conduit_core::{
    kind_id, port_id, present_host_operation_requirement, resource_requirement, ArtifactId,
    CapabilityId, CapabilityLimits, CapabilityOffer, ConfigurationValue, ExecutionProfileId,
    ImplementationId, KindContractRevision, PortDescriptor, PortDirection, PortTemporal,
    PRESENTATION_RESOURCE_CLASS,
};

pub const STATE_COUNT_KIND: &str = "state/count";
pub const STATE_COUNT_VALUE_KIND: &str = "value/count@1";
pub const STATE_COUNT_CONTRACT_REVISION: &str = "conduit.std/state-count@1";
pub const STATE_COUNT_EXECUTION_PROFILE: &str = "conduit.std/state-count-kernel-hosted@1";
pub const STATE_COUNT_IMPLEMENTATION: &str = "std/kernel-state-count@1";
pub const STATE_COUNT_ARTIFACT: &str = "conduit-std-host/state-count@1";
pub const STATE_COUNT_CAPABILITY: &str = "state-count-v1";
pub const CONDUITOS_STATE_COUNT_CAPABILITY: &str = "conduitos-state-count-v1";
pub const CONDUITOS_STATE_COUNT_IMPLEMENTATION: &str = "conduitos/kernel-state-count@1";
pub const CONDUITOS_PORTABLE_STATE_INPUT_PROFILE: &str = "conduitos/portable-state-input-fixed@1";
pub const CONDUITOS_PORTABLE_STATE_INPUT_ARTIFACT: &str = "conduitos/portable-state-input@1";

pub const COUNT_PRESENTATION_KIND: &str = "presentation/count";
pub const COUNT_PRESENTATION_CONTRACT_REVISION: &str = "conduit.std/presentation-count@1";
pub const COUNT_PRESENTATION_EXECUTION_PROFILE: &str =
    "conduit.std/presentation-count-kernel-hosted@1";
pub const COUNT_PRESENTATION_IMPLEMENTATION: &str = "std/kernel-presentation-count@1";
pub const COUNT_PRESENTATION_ARTIFACT: &str = "conduit-std-host/presentation-count@1";
pub const COUNT_PRESENTATION_CAPABILITY: &str = "presentation-count-v1";
pub const COUNT_PRESENTATION_TARGET: &str = "presentation/stdout-count";
pub const COUNT_ENCODED_LEN: u32 = 8;
pub const MAX_COUNT_VALUES: u64 = conduit_time::TIME_EVERY_COUNT + 1;

pub const fn bounded_count_value(start: u64, index: u64) -> Option<u64> {
    if index < MAX_COUNT_VALUES {
        start.checked_add(index)
    } else {
        None
    }
}

pub fn state_count_contract() -> StandardKindContract {
    StandardKindContract {
        kind_id: kind_id(STATE_COUNT_KIND),
        plain_name: "Current count".to_string(),
        summary: "Emit an initial count and one current count after each closing-flow tick."
            .to_string(),
        inputs: vec![PortDescriptor {
            port_id: port_id("bump"),
            value_kind: kind_id(conduit_time::TICK_VALUE_KIND),
            direction: PortDirection::Input,
            temporal: PortTemporal::Flow { closes: true },
        }],
        outputs: vec![PortDescriptor {
            port_id: port_id("value"),
            value_kind: kind_id(STATE_COUNT_VALUE_KIND),
            direction: PortDirection::Output,
            temporal: PortTemporal::Current,
        }],
        configuration: vec![StandardConfigurationField {
            key: "start".to_string(),
            default_value: ConfigurationValue::U64(0),
            rule: StandardConfigurationRule::U64Range {
                minimum: 0,
                maximum: u64::MAX - conduit_time::TIME_EVERY_COUNT,
            },
        }],
        limits: CapabilityLimits {
            max_active_instances: 16,
            max_queue_items: conduit_time::TIME_EVERY_COUNT as u16,
            max_queue_bytes: 64,
        },
        terminal_behavior: TerminalBehavior::CompletesWhenInputsClose,
        hosted_implementation_required: true,
        browser_manifestation_honest: false,
        pico_manifestation_honest: false,
        example: "count: state/count(0)".to_string(),
    }
}

pub fn count_presentation_contract() -> StandardKindContract {
    StandardKindContract {
        kind_id: kind_id(COUNT_PRESENTATION_KIND),
        plain_name: "Count presentation".to_string(),
        summary: "Present up to five exact current count observations on stdout.".to_string(),
        inputs: vec![PortDescriptor {
            port_id: port_id("value"),
            value_kind: kind_id(STATE_COUNT_VALUE_KIND),
            direction: PortDirection::Input,
            temporal: PortTemporal::Current,
        }],
        outputs: Vec::new(),
        configuration: vec![StandardConfigurationField {
            key: "maximum-values".to_string(),
            default_value: ConfigurationValue::U64(MAX_COUNT_VALUES),
            rule: StandardConfigurationRule::U64Range {
                minimum: 1,
                maximum: MAX_COUNT_VALUES,
            },
        }],
        limits: CapabilityLimits {
            max_active_instances: 16,
            max_queue_items: MAX_COUNT_VALUES as u16,
            max_queue_bytes: 64,
        },
        terminal_behavior: TerminalBehavior::CompletesWhenInputsClose,
        hosted_implementation_required: true,
        browser_manifestation_honest: false,
        pico_manifestation_honest: false,
        example: "show: presentation/count".to_string(),
    }
}

pub fn state_count_offer() -> CapabilityOffer {
    offer(
        state_count_contract(),
        OfferIdentity {
            capability: STATE_COUNT_CAPABILITY,
            revision: STATE_COUNT_CONTRACT_REVISION,
            profile: STATE_COUNT_EXECUTION_PROFILE,
            implementation: STATE_COUNT_IMPLEMENTATION,
            artifact: STATE_COUNT_ARTIFACT,
        },
        Vec::new(),
        Vec::new(),
    )
}

pub fn conduitos_state_count_offer() -> CapabilityOffer {
    let mut offer = state_count_offer();
    offer.capability_id = CapabilityId::from(CONDUITOS_STATE_COUNT_CAPABILITY);
    offer.implementation.execution_profile_id =
        ExecutionProfileId::from(CONDUITOS_PORTABLE_STATE_INPUT_PROFILE);
    offer.implementation.implementation_id =
        ImplementationId::from(CONDUITOS_STATE_COUNT_IMPLEMENTATION);
    offer.implementation.artifact_id = ArtifactId::from(CONDUITOS_PORTABLE_STATE_INPUT_ARTIFACT);
    offer
}

pub fn count_presentation_offer() -> CapabilityOffer {
    offer(
        count_presentation_contract(),
        OfferIdentity {
            capability: COUNT_PRESENTATION_CAPABILITY,
            revision: COUNT_PRESENTATION_CONTRACT_REVISION,
            profile: COUNT_PRESENTATION_EXECUTION_PROFILE,
            implementation: COUNT_PRESENTATION_IMPLEMENTATION,
            artifact: COUNT_PRESENTATION_ARTIFACT,
        },
        vec![present_host_operation_requirement(
            kind_id(COUNT_PRESENTATION_TARGET),
            COUNT_ENCODED_LEN,
        )],
        vec![resource_requirement(PRESENTATION_RESOURCE_CLASS, 1)],
    )
}

struct OfferIdentity {
    capability: &'static str,
    revision: &'static str,
    profile: &'static str,
    implementation: &'static str,
    artifact: &'static str,
}

fn offer(
    contract: StandardKindContract,
    identity: OfferIdentity,
    host_operations: Vec<conduit_core::HostOperationRequirement>,
    resources: Vec<conduit_core::ResourceRequirement>,
) -> CapabilityOffer {
    CapabilityOffer {
        startup_parameters: super::startup_face(&contract.configuration),
        shorthand: None,
        capability_id: CapabilityId::from(identity.capability),
        kind_id: contract.kind_id,
        kind_contract_revision: KindContractRevision::from(identity.revision),
        implementation: conduit_core::ImplementationOffer {
            execution_profile_id: ExecutionProfileId::from(identity.profile),
            implementation_id: ImplementationId::from(identity.implementation),
            artifact_id: ArtifactId::from(identity.artifact),
        },
        inputs: contract.inputs,
        outputs: contract.outputs,
        host_operations,
        resource_requirements: resources,
        authority_requirements: Vec::new(),
        limits: contract.limits,
    }
}

#[cfg(feature = "form-catalog")]
pub fn install_count_pipeline_catalogs(
    startup: &mut conduit_form::StartupCatalog,
    profile: &mut conduit_form::ProfileCatalog,
) -> Result<(), String> {
    use conduit_form::{ConfigurationField, ConfigurationRule, KindDefinition, KindSignature};
    for (contract, revision) in [
        (state_count_contract(), STATE_COUNT_CONTRACT_REVISION),
        (
            count_presentation_contract(),
            COUNT_PRESENTATION_CONTRACT_REVISION,
        ),
    ] {
        startup.insert(KindSignature {
            kind: contract.kind_id.as_str().to_string(),
            startup_parameters: contract
                .configuration
                .iter()
                .map(|field| conduit_form::StartupParameterSignature {
                    name: field.key.clone(),
                    value_type: "Count".to_string(),
                    default: Some(match field.default_value {
                        ConfigurationValue::U64(value) => value.to_string(),
                        _ => unreachable!("count family only has Count startup values"),
                    }),
                })
                .collect(),
        })?;
        profile
            .insert(KindDefinition {
                kind_id: contract.kind_id,
                kind_contract_revision: KindContractRevision::from(revision),
                inputs: contract.inputs,
                outputs: contract.outputs,
                configuration: contract
                    .configuration
                    .into_iter()
                    .map(|field| ConfigurationField {
                        key: field.key,
                        default_value: field.default_value,
                        validation: match field.rule {
                            StandardConfigurationRule::U64Range { minimum, maximum } => {
                                ConfigurationRule::U64Range { minimum, maximum }
                            }
                            _ => unreachable!("count family only has Count ranges"),
                        },
                    })
                    .collect(),
            })
            .map_err(|error| error.to_string())?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn count_family_distinguishes_closing_ticks_from_current_counts() {
        let state = state_count_offer();
        assert_eq!(
            state.inputs[0].temporal,
            PortTemporal::Flow { closes: true }
        );
        assert_eq!(state.outputs[0].temporal, PortTemporal::Current);
        assert_eq!(state.startup_parameters[0].name, "start");
        assert!(state.startup_parameters[0].has_default);

        let presentation = count_presentation_offer();
        assert_eq!(presentation.inputs[0].temporal, PortTemporal::Current);
        assert_ne!(state.checked_face(), presentation.checked_face());
    }

    #[cfg(feature = "form-catalog")]
    #[test]
    fn count_family_installs_exact_source_contracts() {
        let mut startup = conduit_form::StartupCatalog::new();
        let mut profile = conduit_form::ProfileCatalog::new();
        conduit_time::install_time_every_catalog(&mut startup, &mut profile).unwrap();
        crate::install_tick_presentation_catalog(&mut startup, &mut profile).unwrap();
        install_count_pipeline_catalogs(&mut startup, &mut profile).unwrap();
        let source = "form count (\n    start: Count = 0\n    bump: Tick...| > value: $Count\n) {\n    gear: state/count(start)\n    bump > gear.bump\n    gear.value > value\n}\nform main {\n    clock: time/every(1s)\n    count: count\n    show: presentation/count\n    clock > count > show\n}\n";
        let syntax = conduit_form::parse_syntax_document(source);
        let checked = conduit_form::check_syntax_document(&syntax, &startup).unwrap();
        let expanded = conduit_form::expand_canonical_form(&checked, "main", &profile).unwrap();
        let count = checked
            .forms
            .iter()
            .find(|form| form.name == "count")
            .unwrap();
        let state = expanded
            .gears
            .iter()
            .find(|operation| operation.kind_id.as_str() == STATE_COUNT_KIND)
            .unwrap();
        assert_eq!(count.checked_face(), state.checked_face());
    }
}
