use alloc::{collections::BTreeMap, format, vec, vec::Vec};
use conduit_core::{
    ArtifactId, BootId, CapabilityId, CapabilityLimits, CapabilityOffer, ConnectionBase,
    ExecutionProfileId, HostAdvertisement, HostId, HostProfileId, ImplementationId,
    KindContractRevision, OfferGeneration, PRESENTATION_RESOURCE_CLASS, PROTOCOL_VERSION, Plan,
    PortDescriptor, PortDirection, PortTemporal, kind_id, port_id, resource_offer,
};
use conduit_form::{
    CanonicalBackCatalog, ProfileCatalog, StartupCatalog, check_syntax_document,
    expand_canonical_form_with_backs, parse_syntax_document,
};
use conduit_planner::{
    PlanningOptions, default_expanded_placements, plan_expanded_canonical_with_options,
};
use conduit_runtime::lowering::{LoweredPlanFragment, lower_plan_fragment};

use super::TEXT_SOURCE_KIND;

pub const FORM_SOURCE: &str = "form conduitos-gear-face {\n source: conduitos.fixture/text-source\n face: patchbay/gear-face\n source.text -> face.subject\n}\n";

pub struct PreparedPresentationPlay {
    pub advertisement: HostAdvertisement,
    pub plan: Plan,
    pub lowered: LoweredPlanFragment,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PreparationError {
    Catalog,
    Form,
    Back,
    Placement,
    Plan,
    Lowering,
}

impl PreparationError {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Catalog => "presentation-catalog-invalid",
            Self::Form => "presentation-form-rejected",
            Self::Back => "presentation-back-rejected",
            Self::Placement => "presentation-placement-rejected",
            Self::Plan => "presentation-plan-rejected",
            Self::Lowering => "presentation-lowering-rejected",
        }
    }
}

pub fn prepare(host: &str, boot: &str) -> Result<PreparedPresentationPlay, PreparationError> {
    let (startup, profile) = catalogs()?;
    let checked = check_syntax_document(&parse_syntax_document(FORM_SOURCE), &startup)
        .map_err(|_| PreparationError::Form)?;
    let mut backs = CanonicalBackCatalog::new();
    conduit_std_catalog::install_patchbay_presentation_backs(&startup, &profile, &mut backs)
        .map_err(|_| PreparationError::Back)?;
    let form = expand_canonical_form_with_backs(&checked, "conduitos-gear-face", &profile, &backs)
        .map_err(|_| PreparationError::Back)?;
    let advertisement = advertisement(host, boot);
    let hosts = [advertisement.clone()];
    let placements =
        default_expanded_placements(&form, &hosts).map_err(|_| PreparationError::Placement)?;
    let plan = plan_expanded_canonical_with_options(
        &form,
        &hosts,
        &placements,
        &[ConnectionBase::Local],
        PlanningOptions {
            connection_bases: &BTreeMap::new(),
            line_candidates: &BTreeMap::new(),
            connection_item_capacity: 1,
            connection_byte_capacity: conduit_presentation::MAX_LAYOUT_FRAME_BYTES as u32,
            authority_grants: &[],
            protected_resource_grants: &[],
            line_offers: &[],
        },
    )
    .map_err(|_| PreparationError::Plan)?;
    if !conduit_core::verify_plan(&plan) || plan.fragments.len() != 1 {
        return Err(PreparationError::Plan);
    }
    let lowered =
        lower_plan_fragment(&plan.fragments[0]).map_err(|_| PreparationError::Lowering)?;
    if !lowered.remote_endpoints.is_empty() {
        return Err(PreparationError::Lowering);
    }
    Ok(PreparedPresentationPlay {
        advertisement,
        plan,
        lowered,
    })
}

fn advertisement(host: &str, boot: &str) -> HostAdvertisement {
    let mut capabilities = conduit_std_catalog::conduitos_presentation_nucleus_offers();
    capabilities.push(text_source_offer());
    HostAdvertisement {
        protocol_version: PROTOCOL_VERSION,
        host_id: HostId::from(host),
        boot_id: BootId::from(boot),
        offer_generation: OfferGeneration(1),
        profile: HostProfileId::from("conduitos/two-lane-cooperative@1"),
        resources: vec![resource_offer(
            &format!("{host}/display"),
            PRESENTATION_RESOURCE_CLASS,
            16,
        )],
        planner_capabilities: Vec::new(),
        capabilities,
    }
}

fn text_source_offer() -> CapabilityOffer {
    CapabilityOffer {
        startup_parameters: Vec::new(),
        shorthand: None,
        capability_id: CapabilityId::from("conduitos-fixture-text-source@1"),
        kind_id: kind_id(TEXT_SOURCE_KIND),
        kind_contract_revision: KindContractRevision::from("conduitos.fixture/text-source@1"),
        implementation: conduit_core::ImplementationOffer {
            execution_profile_id: ExecutionProfileId::from(
                conduit_std_catalog::CONDUITOS_PRESENTATION_PROFILE,
            ),
            implementation_id: ImplementationId::from("conduitos.fixture/text-source@1"),
            artifact_id: ArtifactId::from(conduit_std_catalog::CONDUITOS_PRESENTATION_ARTIFACT),
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

fn catalogs() -> Result<(StartupCatalog, ProfileCatalog), PreparationError> {
    let mut startup = StartupCatalog::new();
    let mut profile = ProfileCatalog::new();
    conduit_std_catalog::install_text_pipeline_catalogs(&mut startup, &mut profile)
        .map_err(|_| PreparationError::Catalog)?;
    conduit_std_catalog::install_layout_catalogs(&mut startup, &mut profile)
        .map_err(|_| PreparationError::Catalog)?;
    conduit_std_catalog::install_presentation_composition_catalogs(&mut startup, &mut profile)
        .map_err(|_| PreparationError::Catalog)?;
    conduit_std_catalog::install_graphics_catalogs(&mut startup, &mut profile)
        .map_err(|_| PreparationError::Catalog)?;
    conduit_std_catalog::install_graphics_presentation_catalogs(&mut startup, &mut profile)
        .map_err(|_| PreparationError::Catalog)?;
    conduit_std_catalog::install_patchbay_presentation_catalogs(&mut startup, &mut profile)
        .map_err(|_| PreparationError::Catalog)?;
    startup
        .insert(conduit_form::KindSignature {
            kind: TEXT_SOURCE_KIND.into(),
            startup_parameters: Vec::new(),
        })
        .map_err(|_| PreparationError::Catalog)?;
    profile
        .insert(conduit_form::KindDefinition {
            kind_id: kind_id(TEXT_SOURCE_KIND),
            kind_contract_revision: KindContractRevision::from("conduitos.fixture/text-source@1"),
            inputs: Vec::new(),
            outputs: text_source_offer().outputs,
            configuration: Vec::new(),
        })
        .map_err(|_| PreparationError::Catalog)?;
    Ok((startup, profile))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plan_is_one_exact_recursive_gear_face_realization() {
        let prepared = prepare("test-host", "test-boot").unwrap();
        let [back] = prepared.plan.realization_backs.as_slice() else {
            panic!("expected one realization Back")
        };
        assert_eq!(
            back.kind_id.as_str(),
            conduit_std_catalog::PATCHBAY_GEAR_FACE_KIND
        );
        assert_eq!(back.invocation_path, "conduitos-gear-face/face");
        assert_ne!(
            prepared.plan.source_document_id.as_str(),
            prepared.plan.checked_form_id.as_str()
        );
        assert_ne!(
            prepared.plan.checked_form_id.as_str(),
            prepared.plan.expanded_form_id.as_str()
        );
        let fragment = &prepared.plan.fragments[0];
        assert_eq!(fragment.placements.len(), 10);
        assert_eq!(fragment.connections.len(), 7);
        assert_eq!(fragment.realization_backs, prepared.plan.realization_backs);
        let manifestation = fragment
            .placements
            .iter()
            .find(|placement| {
                placement.kind_id.as_str() == conduit_std_catalog::GRAPHICS_PRESENTATION_KIND
            })
            .unwrap();
        assert_eq!(
            manifestation.execution_profile_id.as_str(),
            conduit_std_catalog::CONDUITOS_PRESENTATION_PROFILE
        );
        assert_eq!(manifestation.host_operations.len(), 1);
        assert_eq!(manifestation.resources.len(), 1);
    }

    #[test]
    fn missing_back_and_missing_terminal_leaf_are_distinct_failures() {
        let (startup, profile) = catalogs().unwrap();
        let checked = check_syntax_document(&parse_syntax_document(FORM_SOURCE), &startup).unwrap();
        let missing_back = expand_canonical_form_with_backs(
            &checked,
            "conduitos-gear-face",
            &profile,
            &CanonicalBackCatalog::new(),
        )
        .unwrap_err();

        let mut backs = CanonicalBackCatalog::new();
        conduit_std_catalog::install_patchbay_presentation_backs(&startup, &profile, &mut backs)
            .unwrap();
        let expanded =
            expand_canonical_form_with_backs(&checked, "conduitos-gear-face", &profile, &backs)
                .unwrap();
        let mut host = advertisement("test-host", "test-boot");
        host.capabilities.retain(|offer| {
            offer.kind_id.as_str() != conduit_std_catalog::GRAPHICS_PRESENTATION_KIND
        });
        let missing_leaf = default_expanded_placements(&expanded, &[host]).unwrap_err();
        assert_ne!(missing_back.to_string(), missing_leaf.to_string());
    }
}
