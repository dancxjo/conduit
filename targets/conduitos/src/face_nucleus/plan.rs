use alloc::{format, vec, vec::Vec};
use conduit_core::{
    ArtifactId, BaseImplementationId, BootId, CapabilityId, CapabilityLimits, CapabilityOffer,
    ExecutionProfileId, FACE_RESOURCE_CLASS, HostAdvertisement, HostId, HostProfileId,
    ImplementationId, KindIdentity, OfferGeneration, PROTOCOL_VERSION, Plan, PortDescriptor,
    PortDirection, PortTemporal, kind_id, port_id, resource_offer,
};
use conduit_plan_lowering::lowering::{LoweredPlanFragment, lower_plan_fragment};
use conduit_planner::{default_expanded_placements, plan_expanded_canonical};
use conduit_plot::{
    CanonicalBackCatalog, ProfileCatalog, StartupCatalog, check_syntax_document,
    expand_canonical_plot_with_backs, parse_syntax_document,
};

use super::TEXT_SOURCE_KIND;

pub const PLOT_SOURCE: &str = "plot conduitos-gear-front {\n source: conduitos/fixture-text-source\n front: patchbay/gear-front\n source >> front.subject\n}\n";

pub struct PreparedFacePlay {
    pub advertisement: HostAdvertisement,
    pub plan: Plan,
    pub lowered: LoweredPlanFragment,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PreparationError {
    Catalog,
    Plot,
    Back,
    Placement,
    Plan,
    Lowering,
}

impl PreparationError {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Catalog => "face-catalog-invalid",
            Self::Plot => "face-plot-rejected",
            Self::Back => "face-back-rejected",
            Self::Placement => "face-placement-rejected",
            Self::Plan => "face-plan-rejected",
            Self::Lowering => "face-lowering-rejected",
        }
    }
}

pub fn prepare(host: &str, boot: &str) -> Result<PreparedFacePlay, PreparationError> {
    let (startup, profile) = catalogs()?;
    let checked = check_syntax_document(&parse_syntax_document(PLOT_SOURCE), &startup)
        .map_err(|_| PreparationError::Plot)?;
    let mut backs = CanonicalBackCatalog::new();
    conduit_semantic_catalog::install_patchbay_face_backs(&startup, &profile, &mut backs)
        .map_err(|_| PreparationError::Back)?;
    let plot = expand_canonical_plot_with_backs(&checked, "conduitos-gear-front", &profile, &backs)
        .map_err(|_| PreparationError::Back)?;
    let advertisement = advertisement(host, boot);
    let hosts = [advertisement.clone()];
    let placements =
        default_expanded_placements(&plot, &hosts).map_err(|_| PreparationError::Placement)?;
    let plan = plan_expanded_canonical(
        &plot,
        &hosts,
        &placements,
        &[BaseImplementationId::from("conduit.base/local@1")],
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
    Ok(PreparedFacePlay {
        advertisement,
        plan,
        lowered,
    })
}

fn advertisement(host: &str, boot: &str) -> HostAdvertisement {
    let mut capabilities = super::face_nucleus_offers();
    capabilities.push(text_source_offer());
    HostAdvertisement {
        protocol_version: PROTOCOL_VERSION,
        host_id: HostId::from(host),
        boot_id: BootId::from(boot),
        offer_generation: OfferGeneration(1),
        profile: HostProfileId::from("conduitos/two-lane-cooperative@1"),
        bases: vec![],
        resources: vec![resource_offer(
            &format!("{host}/display"),
            FACE_RESOURCE_CLASS,
            16,
        )],
        planner_capabilities: Vec::new(),
        capabilities,
    }
}

fn text_source_offer() -> CapabilityOffer {
    conduit_core::capability_offer_from_parts! {
        semantic_contract: Default::default(),
        startup_parameters: Vec::new(),
        shorthand: None,
        capability_id: CapabilityId::from("conduitos-fixture-text-source@1"),
        kind_id: kind_id(TEXT_SOURCE_KIND),
        kind_contract_revision: KindIdentity::from("conduitos/fixture-text-source@1"),
        implementation: conduit_core::ImplementationOffer {
            execution_profile_id: ExecutionProfileId::from(super::CONDUITOS_FACE_PROFILE),
            implementation_id: ImplementationId::from("conduitos.fixture/text-source@1"),
            artifact_id: ArtifactId::from(super::CONDUITOS_FACE_ARTIFACT),
        },
        inputs: Vec::new(),
        outputs: vec![PortDescriptor {
            port_id: port_id("text"),
            value_kind: kind_id(conduit_semantic_catalog::TEXT_FACE_VALUE_KIND),
            direction: PortDirection::Output,
            temporal: PortTemporal::Value,
            abnormal_kind: None,
        }],
        host_calls: Vec::new(),
        resource_requirements: Vec::new(),
        authority_requirements: Vec::new(),
        limits: CapabilityLimits {
            max_active_instances: 1,
            max_queue_items: 1,
            max_queue_bytes: conduit_text::MAX_TEXT_BYTES,
        },
    }
}

fn catalogs() -> Result<(StartupCatalog, ProfileCatalog), PreparationError> {
    let mut startup = StartupCatalog::new();
    let mut profile = ProfileCatalog::new();
    conduit_semantic_catalog::install_text_pipeline_catalogs(&mut startup, &mut profile)
        .map_err(|_| PreparationError::Catalog)?;
    conduit_semantic_catalog::install_layout_catalogs(&mut startup, &mut profile)
        .map_err(|_| PreparationError::Catalog)?;
    conduit_semantic_catalog::install_face_composition_catalogs(&mut startup, &mut profile)
        .map_err(|_| PreparationError::Catalog)?;
    conduit_semantic_catalog::install_graphics_catalogs(&mut startup, &mut profile)
        .map_err(|_| PreparationError::Catalog)?;
    conduit_semantic_catalog::install_graphics_face_catalog(&mut startup, &mut profile)
        .map_err(|_| PreparationError::Catalog)?;
    conduit_face::install_bitmap_face_catalog(&mut startup, &mut profile)
        .map_err(|_| PreparationError::Catalog)?;
    conduit_semantic_catalog::install_patchbay_face_catalogs(&mut startup, &mut profile)
        .map_err(|_| PreparationError::Catalog)?;
    startup
        .insert(conduit_plot::KindSignature {
            kind: TEXT_SOURCE_KIND.into(),
            startup_parameters: Vec::new(),
        })
        .map_err(|_| PreparationError::Catalog)?;
    profile
        .insert(conduit_plot::KindProjection {
            kind_id: kind_id(TEXT_SOURCE_KIND),
            kind_contract_revision: KindIdentity::from("conduitos/fixture-text-source@1"),
            inputs: Vec::new(),
            outputs: text_source_offer().outputs,
            configuration: Default::default(),
        })
        .map_err(|_| PreparationError::Catalog)?;
    Ok((startup, profile))
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::string::ToString;

    #[test]
    fn plan_is_one_exact_recursive_gear_front_realization() {
        let prepared = prepare("test-host", "test-boot").unwrap();
        let [back] = prepared.plan.realization_backs.as_slice() else {
            panic!("expected one realization Back")
        };
        assert_eq!(
            back.kind_id.as_str(),
            conduit_semantic_catalog::PATCHBAY_GEAR_FACE_KIND
        );
        assert_eq!(back.invocation_path, "conduitos-gear-front/front");
        assert_ne!(
            prepared.plan.source_document_id.as_str(),
            prepared.plan.checked_plot_id.as_str()
        );
        assert_ne!(
            prepared.plan.checked_plot_id.as_str(),
            prepared.plan.expanded_plot_id.as_str()
        );
        let fragment = &prepared.plan.fragments[0];
        assert_eq!(fragment.placements.len(), 11);
        assert_eq!(fragment.connections.len(), 8);
        assert!(fragment.placements.iter().any(|placement| {
            placement.kind_id.as_str() == conduit_semantic_catalog::LAYOUT_INSET_KIND
                && placement.implementation_id.as_str() == "conduitos/layout/inset-implementation@1"
        }));
        assert_eq!(fragment.realization_backs, prepared.plan.realization_backs);
        let manifestation = fragment
            .placements
            .iter()
            .find(|placement| {
                placement.kind_id.as_str() == conduit_semantic_catalog::GRAPHICS_FACE_KIND
            })
            .unwrap();
        assert_eq!(
            manifestation.execution_profile_id.as_str(),
            crate::face_nucleus::CONDUITOS_FACE_PROFILE
        );
        assert_eq!(manifestation.host_calls.len(), 1);
        assert_eq!(manifestation.resources.len(), 1);
    }

    #[test]
    fn missing_back_and_missing_terminal_leaf_are_distinct_failures() {
        let (startup, profile) = catalogs().unwrap();
        let checked = check_syntax_document(&parse_syntax_document(PLOT_SOURCE), &startup).unwrap();
        let unexpanded = expand_canonical_plot_with_backs(
            &checked,
            "conduitos-gear-front",
            &profile,
            &CanonicalBackCatalog::new(),
        )
        .unwrap();
        let mut host_without_patchbay_direct = advertisement("test-host", "test-boot");
        host_without_patchbay_direct.capabilities.retain(|offer| {
            !matches!(
                offer.kind_id.as_str(),
                conduit_semantic_catalog::PATCHBAY_FACE_KIND
                    | conduit_semantic_catalog::PATCHBAY_PORT_KIND
                    | conduit_semantic_catalog::PATCHBAY_CORD_KIND
            )
        });
        let missing_back =
            default_expanded_placements(&unexpanded, &[host_without_patchbay_direct.clone()])
                .unwrap_err();

        let mut backs = CanonicalBackCatalog::new();
        conduit_semantic_catalog::install_patchbay_face_backs(&startup, &profile, &mut backs)
            .unwrap();
        let expanded =
            expand_canonical_plot_with_backs(&checked, "conduitos-gear-front", &profile, &backs)
                .unwrap();
        let mut host = host_without_patchbay_direct;
        host.capabilities
            .retain(|offer| offer.kind_id.as_str() != conduit_semantic_catalog::GRAPHICS_FACE_KIND);
        let missing_leaf = default_expanded_placements(&expanded, &[host]).unwrap_err();
        assert_ne!(missing_back.to_string(), missing_leaf.to_string());
    }
}
