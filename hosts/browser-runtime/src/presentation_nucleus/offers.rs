use super::{
    offer_composition, BROWSER_PRESENTATION_ARTIFACT, BROWSER_PRESENTATION_PROFILE,
    FIXTURE_GRAPHICS_KIND, FIXTURE_LAYOUT_KIND, FIXTURE_PRESENT_OPERATION, FIXTURE_TEXT_KIND,
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
    offer_composition::offers()
}

pub fn human_io_offers() -> Vec<CapabilityOffer> {
    let mut offers = offers();
    offers.extend(crate::human_media::browser_media_acquisition_offers());
    offers.push(crate::human_media::browser_camera_frame_sink_offer());
    offers
}

pub fn human_io_advertisement_offers() -> Vec<CapabilityOffer> {
    let mut advertisement = crate::human_media::browser_media_acquisition_offers();
    advertisement.extend(offers().into_iter().filter(|offer| {
        matches!(
            offer.kind_id.as_str(),
            conduit_std_catalog::TEXT_PRESENTATION_KIND
                | conduit_std_catalog::GRAPHICS_RECT_KIND
                | conduit_std_catalog::GRAPHICS_TEXT_KIND
                | conduit_std_catalog::GRAPHICS_ICON_KIND
        )
    }));
    advertisement.push(crate::human_media::browser_camera_frame_sink_offer());
    advertisement.sort_by(|left, right| left.capability_id.cmp(&right.capability_id));
    advertisement
}

#[cfg(test)]
pub(super) fn canonical_offer(kind: &str) -> Option<CapabilityOffer> {
    offer_composition::portable_offer(kind)
        .or_else(|| {
            (kind == conduit_std_catalog::TEXT_PRESENTATION_KIND)
                .then(offer_composition::text_offer)
        })
        .or_else(|| (kind == conduit_text::TEXT_UPPER_KIND).then(super::browser_text_upper_offer))
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
    let text_presentation = offers()
        .into_iter()
        .find(|offer| offer.kind_id.as_str() == conduit_std_catalog::TEXT_PRESENTATION_KIND)
        .expect("browser text presentation offer is installed");
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
        capabilities: vec![
            super::browser_text_upper_offer(),
            text_presentation,
            text_source_offer(),
        ],
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
            max_queue_bytes: conduit_text::MAX_TEXT_BYTES,
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

pub(super) fn fixture_startup_catalog() -> Result<conduit_form::StartupCatalog, String> {
    let mut startup = conduit_form::StartupCatalog::new();
    let mut profiles = conduit_form::ProfileCatalog::new();
    conduit_std_catalog::install_layout_catalogs(&mut startup, &mut profiles)?;
    conduit_std_catalog::install_presentation_composition_catalogs(&mut startup, &mut profiles)?;
    conduit_std_catalog::install_graphics_catalogs(&mut startup, &mut profiles)?;
    for kind in [FIXTURE_GRAPHICS_KIND, FIXTURE_LAYOUT_KIND] {
        startup
            .insert(conduit_form::KindSignature {
                kind: kind.to_string(),
                startup_parameters: Vec::new(),
            })
            .map_err(|error| format!("install browser fixture startup signature: {error}"))?;
    }
    Ok(startup)
}

pub(super) fn text_fixture_catalog() -> Result<conduit_form::ProfileCatalog, String> {
    let mut catalog = conduit_form::ProfileCatalog::new();
    let mut startup = conduit_form::StartupCatalog::new();
    conduit_std_catalog::install_text_pipeline_catalogs(&mut startup, &mut catalog)?;
    catalog
        .insert(conduit_form::KindDefinition {
            kind_id: kind_id(FIXTURE_TEXT_KIND),
            kind_contract_revision: KindContractRevision::from("browser.fixture/text-source@1"),
            inputs: Vec::new(),
            outputs: text_source_offer().outputs,
            configuration: Vec::new(),
        })
        .map_err(|error| format!("install browser text source: {error:?}"))?;
    Ok(catalog)
}

pub(super) fn text_fixture_startup_catalog() -> Result<conduit_form::StartupCatalog, String> {
    let mut startup = conduit_form::StartupCatalog::new();
    let mut profiles = conduit_form::ProfileCatalog::new();
    conduit_std_catalog::install_text_pipeline_catalogs(&mut startup, &mut profiles)?;
    startup
        .insert(conduit_form::KindSignature {
            kind: FIXTURE_TEXT_KIND.to_string(),
            startup_parameters: Vec::new(),
        })
        .map_err(|error| format!("install browser text fixture startup signature: {error}"))?;
    Ok(startup)
}

#[cfg(test)]
mod ownership_tests {
    use super::*;

    #[test]
    fn browser_human_io_is_portable_and_contains_no_renderer_kind() {
        let offers = human_io_offers();
        assert!(offers.iter().any(|offer| {
            offer.kind_id.as_str() == conduit_std_catalog::TEXT_PRESENTATION_KIND
        }));
        assert!(offers
            .iter()
            .any(|offer| offer.kind_id.as_str() == conduit_std_catalog::GRAPHICS_RECT_KIND));
        assert!(!offers.iter().any(|offer| {
            let kind = offer.kind_id.as_str();
            kind.contains("dom") || kind.contains("canvas")
        }));
        assert!(offers
            .iter()
            .all(|offer| { offer.kind_id.as_str() != conduit_std_catalog::CAMERA_SOURCE_KIND }));
    }
}
