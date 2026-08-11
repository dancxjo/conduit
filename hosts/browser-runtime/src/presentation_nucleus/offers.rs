use super::{
    BROWSER_PRESENTATION_ARTIFACT, BROWSER_PRESENTATION_PROFILE, FIXTURE_GRAPHICS_KIND,
    FIXTURE_LAYOUT_KIND, FIXTURE_PRESENT_OPERATION, FIXTURE_TEXT_KIND,
};
use conduit_core::{
    kind_id, port_id, ArtifactId, CapabilityLimits, CapabilityOffer, ExecutionProfileId,
    HostAdvertisement, HostId, HostOperationContractId, HostOperationRequirement, HostProfileId,
    ImplementationId, KindContractRevision, OfferGeneration, PortDescriptor, PortDirection,
    PortTemporal, PROTOCOL_VERSION,
};
use conduit_presentation::{
    MAX_GRAPHICS_SCENE_BYTES, MAX_LAYOUT_FRAME_BYTES, MAX_PRESENTATION_COMPOSITION_BYTES,
};

pub fn offers() -> Vec<CapabilityOffer> {
    conduit_std_catalog::browser_presentation_nucleus_offers()
}

#[cfg(test)]
pub(super) fn canonical_offer(kind: &str) -> Option<CapabilityOffer> {
    conduit_std_catalog::layout_offer_for(kind)
        .or_else(|| conduit_std_catalog::presentation_composition_offer_for(kind))
        .or_else(|| conduit_std_catalog::graphics_offer_for(kind))
        .or_else(|| {
            (kind == conduit_std_catalog::TEXT_PRESENTATION_KIND)
                .then(conduit_std_catalog::text_presentation_offer)
        })
}

pub(super) fn advertisement() -> HostAdvertisement {
    let mut capabilities = offers();
    capabilities.push(fixture_offer(
        FIXTURE_GRAPHICS_KIND,
        conduit_presentation::GRAPHICS_SCENE_KIND,
        MAX_GRAPHICS_SCENE_BYTES as u32,
    ));
    capabilities.push(fixture_offer(
        FIXTURE_LAYOUT_KIND,
        conduit_presentation::LAYOUT_FRAME_KIND,
        MAX_LAYOUT_FRAME_BYTES as u32,
    ));
    HostAdvertisement {
        protocol_version: PROTOCOL_VERSION,
        host_id: HostId::from("browser-presentation-host"),
        boot_id: conduit_core::BootId::from("browser-presentation-boot"),
        offer_generation: OfferGeneration(1),
        profile: HostProfileId::from("browser/presentation-nucleus@1"),
        resources: Vec::new(),
        planner_capabilities: Vec::new(),
        capabilities,
    }
}

pub(super) fn text_advertisement() -> HostAdvertisement {
    let text_offer = offers()
        .into_iter()
        .find(|offer| offer.kind_id.as_str() == conduit_std_catalog::TEXT_PRESENTATION_KIND)
        .expect("browser text offer is installed");
    HostAdvertisement {
        protocol_version: PROTOCOL_VERSION,
        host_id: HostId::from("browser-presentation-host"),
        boot_id: conduit_core::BootId::from("browser-presentation-boot"),
        offer_generation: OfferGeneration(1),
        profile: HostProfileId::from("browser/presentation-nucleus@1"),
        resources: vec![conduit_core::resource_offer(
            "browser-presentation-slot",
            conduit_core::PRESENTATION_RESOURCE_CLASS,
            1,
        )],
        planner_capabilities: Vec::new(),
        capabilities: vec![text_source_offer(), text_offer],
    }
}

fn text_source_offer() -> CapabilityOffer {
    CapabilityOffer {
        startup_parameters: Vec::new(),
        shorthand: None,
        capability_id: conduit_core::CapabilityId::from("browser-fixture-text-source@1"),
        kind_id: kind_id(FIXTURE_TEXT_KIND),
        kind_contract_revision: KindContractRevision::from("browser.fixture/text-source@1"),
        implementation: conduit_core::ImplementationOffer {
            execution_profile_id: ExecutionProfileId::from(BROWSER_PRESENTATION_PROFILE),
            implementation_id: ImplementationId::from("browser.fixture/text-source@1"),
            artifact_id: ArtifactId::from(BROWSER_PRESENTATION_ARTIFACT),
        },
        inputs: Vec::new(),
        outputs: vec![PortDescriptor {
            port_id: port_id("text"),
            value_kind: kind_id(conduit_std_catalog::TEXT_PRESENTATION_VALUE_KIND),
            direction: PortDirection::Output,
            temporal: PortTemporal::Value,
        }],
        host_operations: Vec::new(),
        resource_requirements: Vec::new(),
        authority_requirements: Vec::new(),
        limits: CapabilityLimits {
            max_active_instances: 1,
            max_queue_items: 1,
            max_queue_bytes: conduit_std_catalog::MAX_TEXT_BYTES,
        },
    }
}

fn fixture_offer(kind: &str, value_kind: &str, maximum_bytes: u32) -> CapabilityOffer {
    CapabilityOffer {
        startup_parameters: Vec::new(),
        shorthand: None,
        capability_id: conduit_core::CapabilityId::from(format!("{kind}-capability@1").as_str()),
        kind_id: kind_id(kind),
        kind_contract_revision: KindContractRevision::from("browser.fixture/presentation-sink@1"),
        implementation: conduit_core::ImplementationOffer {
            execution_profile_id: ExecutionProfileId::from(BROWSER_PRESENTATION_PROFILE),
            implementation_id: ImplementationId::from(format!("{kind}-implementation@1").as_str()),
            artifact_id: ArtifactId::from(BROWSER_PRESENTATION_ARTIFACT),
        },
        inputs: vec![PortDescriptor {
            port_id: port_id("input"),
            value_kind: kind_id(value_kind),
            direction: PortDirection::Input,
            temporal: PortTemporal::Value,
        }],
        outputs: Vec::new(),
        host_operations: vec![HostOperationRequirement {
            contract_id: HostOperationContractId::from(FIXTURE_PRESENT_OPERATION),
            target_kind: Some(kind_id(kind)),
            maximum_in_flight: 1,
            maximum_input_bytes: maximum_bytes,
            maximum_output_bytes: 0,
        }],
        resource_requirements: Vec::new(),
        authority_requirements: Vec::new(),
        limits: CapabilityLimits {
            max_active_instances: 1,
            max_queue_items: 1,
            max_queue_bytes: maximum_bytes.max(MAX_PRESENTATION_COMPOSITION_BYTES as u32),
        },
    }
}

pub(super) fn fixture_catalog() -> Result<conduit_form::ProfileCatalog, String> {
    let mut catalog = conduit_std_catalog::standard_profile_catalog();
    let mut startup = conduit_form::StartupCatalog::new();
    conduit_std_catalog::install_layout_catalogs(&mut startup, &mut catalog)?;
    conduit_std_catalog::install_presentation_composition_catalogs(&mut startup, &mut catalog)?;
    conduit_std_catalog::install_graphics_catalogs(&mut startup, &mut catalog)?;
    for (kind, value_kind) in [
        (
            FIXTURE_GRAPHICS_KIND,
            conduit_presentation::GRAPHICS_SCENE_KIND,
        ),
        (FIXTURE_LAYOUT_KIND, conduit_presentation::LAYOUT_FRAME_KIND),
    ] {
        catalog
            .insert(conduit_form::KindDefinition {
                kind_id: kind_id(kind),
                kind_contract_revision: KindContractRevision::from(
                    "browser.fixture/presentation-sink@1",
                ),
                inputs: vec![PortDescriptor {
                    port_id: port_id("input"),
                    value_kind: kind_id(value_kind),
                    direction: PortDirection::Input,
                    temporal: PortTemporal::Value,
                }],
                outputs: Vec::new(),
                configuration: Vec::new(),
            })
            .map_err(|error| format!("install browser fixture sink: {error:?}"))?;
    }
    Ok(catalog)
}

pub(super) fn text_fixture_catalog() -> Result<conduit_form::ProfileCatalog, String> {
    let mut catalog = conduit_form::ProfileCatalog::new();
    catalog
        .insert(conduit_form::KindDefinition {
            kind_id: kind_id(FIXTURE_TEXT_KIND),
            kind_contract_revision: KindContractRevision::from("browser.fixture/text-source@1"),
            inputs: Vec::new(),
            outputs: text_source_offer().outputs,
            configuration: Vec::new(),
        })
        .map_err(|error| format!("install browser text source: {error:?}"))?;
    catalog
        .insert(conduit_form::KindDefinition {
            kind_id: kind_id(conduit_std_catalog::TEXT_PRESENTATION_KIND),
            kind_contract_revision: KindContractRevision::from(
                conduit_std_catalog::TEXT_PRESENTATION_CONTRACT_REVISION,
            ),
            inputs: conduit_std_catalog::text_presentation_inputs(),
            outputs: Vec::new(),
            configuration: vec![conduit_form::ConfigurationField {
                key: "maximum-values".into(),
                default_value: conduit_core::ConfigurationValue::U64(
                    conduit_std_catalog::MAX_TEXT_VALUES,
                ),
                validation: conduit_form::ConfigurationRule::U64Range {
                    minimum: 1,
                    maximum: conduit_std_catalog::MAX_TEXT_VALUES,
                },
            }],
        })
        .map_err(|error| format!("install browser text presentation: {error:?}"))?;
    Ok(catalog)
}
