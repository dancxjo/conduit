//! Portable event, retained-state, and semantic-view boundary for applications.

use super::{
    KindConfigurationField, KindConfigurationRule, KindTerminalBehavior, StandardKindContract,
};
use alloc::{string::ToString, vec, vec::Vec};
use conduit_core::{
    kind_id, port_id, CapabilityLimits, ConfigurationValue, KindIdentity, PortDescriptor,
    PortDirection, PortTemporal,
};
use conduit_presentation::{
    APPLICATION_EVENT_INFO_ID, APPLICATION_VIEW_INFO_ID, MAX_APPLICATION_EVENT_ENCODED_BYTES,
    MAX_APPLICATION_VIEW_BYTES,
};

pub const APPLICATION_EVENT_SOURCE_KIND: &str = "input/application-event";
pub const RETAINED_APPLICATION_KIND: &str = "application/retained";
pub const APPLICATION_VIEW_PRESENTATION_KIND: &str = "presentation/application-view";
pub const APPLICATION_EVENT_PORT: &str = "event";
pub const APPLICATION_VIEW_PORT: &str = "view";
pub const APPLICATION_ID_CONFIGURATION: &str = "application";
pub const APPLICATION_CONTRACT_REVISION: &str = "conduit.application/event-view@1";

fn limits(bytes: usize) -> CapabilityLimits {
    CapabilityLimits {
        max_active_instances: 4,
        max_queue_items: 8,
        max_queue_bytes: bytes as u32,
    }
}

pub fn application_contracts() -> Vec<StandardKindContract> {
    vec![
        event_source_contract(),
        retained_application_contract(),
        view_presentation_contract(),
    ]
}

pub fn event_source_contract() -> StandardKindContract {
    StandardKindContract {
        kind_id: kind_id(APPLICATION_EVENT_SOURCE_KIND),
        plain_name: "Application events".into(),
        summary: "Produce a bounded flow of portable, revision-bound application events.".into(),
        inputs: Vec::new(),
        outputs: vec![PortDescriptor {
            port_id: port_id(APPLICATION_EVENT_PORT),
            value_kind: kind_id(APPLICATION_EVENT_INFO_ID),
            direction: PortDirection::Output,
            temporal: PortTemporal::Flow { closes: false },
        }],
        configuration: Default::default(),
        limits: limits(MAX_APPLICATION_EVENT_ENCODED_BYTES),
        terminal_behavior: KindTerminalBehavior::HostInputEndsOrFailsSource,
        hosted_implementation_required: true,
        browser_manifestation_honest: true,
        pico_manifestation_honest: false,
        example: "events: input/application-event".into(),
    }
}

pub fn retained_application_contract() -> StandardKindContract {
    StandardKindContract {
        kind_id: kind_id(RETAINED_APPLICATION_KIND), plain_name: "Retained application".into(),
        summary: "Apply revision-bound events to finite retained application state and emit semantic views.".into(),
        inputs: vec![PortDescriptor { port_id: port_id(APPLICATION_EVENT_PORT), value_kind: kind_id(APPLICATION_EVENT_INFO_ID), direction: PortDirection::Input, temporal: PortTemporal::Flow { closes: false } }],
        outputs: vec![PortDescriptor { port_id: port_id(APPLICATION_VIEW_PORT), value_kind: kind_id(APPLICATION_VIEW_INFO_ID), direction: PortDirection::Output, temporal: PortTemporal::Flow { closes: false } }],
        configuration: vec![KindConfigurationField { key: APPLICATION_ID_CONFIGURATION.into(), default_value: ConfigurationValue::Text("application".into()), rule: KindConfigurationRule::TextBytes { maximum: 32 } }],
        limits: limits(MAX_APPLICATION_VIEW_BYTES), terminal_behavior: KindTerminalBehavior::CompletesWhenInputsClose,
        hosted_implementation_required: true, browser_manifestation_honest: true, pico_manifestation_honest: false,
        example: "app: application/retained(application = \"tour\")".into(),
    }
}

pub fn view_presentation_contract() -> StandardKindContract {
    StandardKindContract {
        kind_id: kind_id(APPLICATION_VIEW_PRESENTATION_KIND),
        plain_name: "Application view".into(),
        summary:
            "Present bounded renderer-neutral application views without owning application state."
                .into(),
        inputs: vec![PortDescriptor {
            port_id: port_id(APPLICATION_VIEW_PORT),
            value_kind: kind_id(APPLICATION_VIEW_INFO_ID),
            direction: PortDirection::Input,
            temporal: PortTemporal::Flow { closes: false },
        }],
        outputs: Vec::new(),
        configuration: Default::default(),
        limits: limits(MAX_APPLICATION_VIEW_BYTES),
        terminal_behavior: KindTerminalBehavior::PresentsEachFieldAndCompletesWhenInputCloses,
        hosted_implementation_required: true,
        browser_manifestation_honest: true,
        pico_manifestation_honest: false,
        example: "view: presentation/application-view".into(),
    }
}

#[cfg(feature = "form-catalog")]
pub fn install_application_catalogs(
    startup: &mut conduit_form::StartupCatalog,
    profile: &mut conduit_form::ProfileCatalog,
) -> Result<(), alloc::string::String> {
    use conduit_form::{
        KindConfigurationField, KindConfigurationRule, KindProjection, KindSignature,
        StartupParameterSignature,
    };
    for contract in application_contracts() {
        startup.insert(KindSignature {
            kind: contract.kind_id.as_str().to_string(),
            startup_parameters: contract
                .configuration
                .iter()
                .map(|field| StartupParameterSignature {
                    name: field.key.clone(),
                    value_type: "Text".to_string(),
                    default: match &field.default_value {
                        ConfigurationValue::Text(value) => Some(alloc::format!("{value:?}")),
                        _ => None,
                    },
                })
                .collect(),
        })?;
        profile
            .insert(KindProjection {
                kind_id: contract.kind_id,
                kind_contract_revision: KindIdentity::from(APPLICATION_CONTRACT_REVISION),
                inputs: contract.inputs,
                outputs: contract.outputs,
                configuration: contract
                    .configuration
                    .into_iter()
                    .map(|field| KindConfigurationField {
                        key: field.key,
                        default_value: field.default_value,
                        rule: match field.rule {
                            KindConfigurationRule::Any => KindConfigurationRule::Any,
                            KindConfigurationRule::U64Range { minimum, maximum } => {
                                KindConfigurationRule::U64Range { minimum, maximum }
                            }
                            KindConfigurationRule::TextBytes { maximum } => {
                                KindConfigurationRule::TextBytes { maximum }
                            }
                            _ => KindConfigurationRule::Any,
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
    fn seam_is_exactly_typed_and_finite() {
        let contracts = application_contracts();
        assert_eq!(
            contracts[0].outputs[0].value_kind.as_str(),
            APPLICATION_EVENT_INFO_ID
        );
        assert_eq!(
            contracts[1].inputs[0].port_id,
            contracts[0].outputs[0].port_id
        );
        assert_eq!(
            contracts[1].inputs[0].value_kind,
            contracts[0].outputs[0].value_kind
        );
        assert_eq!(
            contracts[1].outputs[0].port_id,
            contracts[2].inputs[0].port_id
        );
        assert_eq!(
            contracts[1].outputs[0].value_kind,
            contracts[2].inputs[0].value_kind
        );
        assert_eq!(contracts[1].limits.max_queue_items, 8);
        assert_eq!(
            contracts[1].limits.max_queue_bytes,
            MAX_APPLICATION_VIEW_BYTES as u32
        );
    }
}
