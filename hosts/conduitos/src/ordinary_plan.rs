//! Current Form check, exact boot-scoped planning, and numeric lowering.

use alloc::{format, vec, vec::Vec};

use conduit_core::{
    ActivePlayIdentity, ArtifactId, BaseImplementationId, BootId, CapabilityId, ExecutionProfileId,
    HostAdvertisement, HostId, HostProfileId, ImplementationId, OfferGeneration, PROTOCOL_VERSION,
    Plan, PlanId, ResourceOffer, bind_active_play, resource_offer,
};
use conduit_plan_lowering::lowering::lower_plan_fragment;
use conduit_planner::{
    PlanningOptions, default_expanded_placements, plan_expanded_canonical_with_options,
};

use crate::{
    execution_region::{seal_execution_region, validate_execution_region},
    identity::{BootIdentities, hex as hex_identity},
    offer::{CAPABILITY_COUNT, HostOffer},
    text_planned_kernel::TextPlannedKernel,
};

pub const ORDINARY_FORM_SOURCE: &str = "form conduitos-text-upper {\n    upper: text/upper\n    show: presentation/text\n    \"Hello, ConduitOS\" > upper > show\n}\n";
pub const TEXT_LITERAL: &str = "Hello, ConduitOS";
pub const TEXT_RESULT: &str = "HELLO, CONDUITOS";
const CORD_BYTES: u32 = conduit_text::MAX_TEXT_BYTES;
const ORDINARY_PLACEMENT_COUNT: usize = 3;
pub const COOPERATIVE_REGION_PROFILE: &str = "conduitos/cooperative-bounded-step@1";

pub struct PreparedOrdinaryPlay {
    pub kernel: TextPlannedKernel,
    pub advertisement: HostAdvertisement,
    pub plan: Plan,
    pub source_document_id: conduit_core::SourceDocumentId,
    pub checked_form_id: conduit_core::CheckedFormId,
    pub expanded_form_id: conduit_core::ExpandedFormId,
    pub plan_id: PlanId,
    pub fragment_id: conduit_core::FragmentId,
    pub active_play: ActivePlayIdentity,
    pub planned_sign_items: u16,
    pub planned_sign_bytes: u32,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PreparationError {
    OfferMismatch,
    FormRejected,
    PlacementRejected,
    PlanRejected,
    LoweringRejected,
    KernelRejected,
}

impl PreparationError {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::OfferMismatch => "ordinary-offer-mismatch",
            Self::FormRejected => "ordinary-form-rejected",
            Self::PlacementRejected => "ordinary-placement-rejected",
            Self::PlanRejected => "ordinary-plan-rejected",
            Self::LoweringRejected => "ordinary-lowering-rejected",
            Self::KernelRejected => "ordinary-kernel-rejected",
        }
    }
}

pub fn prepare(
    identities: &BootIdentities,
    fixed_offer: &HostOffer<'_>,
    build_id: &str,
) -> Result<PreparedOrdinaryPlay, PreparationError> {
    let advertisement = advertisement(identities, fixed_offer, build_id)?;
    let form = crate::ordinary_form::checked_expanded_text_form(ORDINARY_FORM_SOURCE)?;
    validate_text_capacity(&form, CORD_BYTES)?;
    let hosts = [advertisement.clone()];
    let placements = default_expanded_placements(&form, &hosts)
        .map_err(|_| PreparationError::PlacementRejected)?;
    let plan = plan_expanded_canonical_with_options(
        &form,
        &hosts,
        &placements,
        &[BaseImplementationId::from("conduit.base/local@1")],
        PlanningOptions {
            connection_bases: &alloc::collections::BTreeMap::new(),
            line_candidates: &alloc::collections::BTreeMap::new(),
            connection_item_capacity: 1,
            connection_byte_capacity: CORD_BYTES,
            authority_grants: &[],
            protected_resource_grants: &[],
            line_offers: &[],
        },
    )
    .map_err(|_| PreparationError::PlanRejected)?;
    let plan = seal_execution_region(plan, &advertisement, fixed_offer)?;
    if !conduit_core::verify_plan(&plan) || plan.fragments.len() != 1 {
        return Err(PreparationError::PlanRejected);
    }
    let fragment = &plan.fragments[0];
    validate_execution_region(fragment, &advertisement, fixed_offer)?;
    if fragment.host_id != hosts[0].host_id
        || fragment.boot_id != hosts[0].boot_id
        || fragment.offer_generation != hosts[0].offer_generation
        || fragment.placements.len() != ORDINARY_PLACEMENT_COUNT
        || fragment.placements.iter().any(|placement| {
            placement.host_id != hosts[0].host_id || placement.boot_id != hosts[0].boot_id
        })
    {
        return Err(PreparationError::PlanRejected);
    }
    let lowered = lower_plan_fragment(fragment).map_err(|_| PreparationError::LoweringRejected)?;
    if lowered.sign_items > fixed_offer.sign_item_capacity
        || lowered.cord_value_slots > 2
        || lowered.cord_value_bytes > CORD_BYTES * 2
    {
        return Err(PreparationError::PlanRejected);
    }
    let kernel = TextPlannedKernel::prepare(fragment, &lowered)
        .map_err(|_| PreparationError::KernelRejected)?;
    let active_play = bind_active_play(&plan.plan_id, &fragment.host_id, &fragment.boot_id, 0);
    Ok(PreparedOrdinaryPlay {
        kernel,
        advertisement,
        source_document_id: plan.source_document_id.clone(),
        checked_form_id: plan.checked_form_id.clone(),
        expanded_form_id: plan.expanded_form_id.clone(),
        plan_id: plan.plan_id.clone(),
        fragment_id: fragment.fragment_id.clone(),
        active_play,
        planned_sign_items: lowered.sign_items,
        planned_sign_bytes: lowered.sign_bytes,
        plan,
    })
}

pub(crate) fn advertisement(
    identities: &BootIdentities,
    fixed: &HostOffer<'_>,
    build_id: &str,
) -> Result<HostAdvertisement, PreparationError> {
    fixed
        .validate()
        .map_err(|_| PreparationError::OfferMismatch)?;
    if fixed.host_id != identities.host
        || fixed.boot_id != identities.boot
        || fixed.generation == 0
        || fixed.capabilities.len() != CAPABILITY_COUNT
        || fixed.capabilities[2].kind != conduit_text::TEXT_LITERAL_KIND
        || fixed.capabilities[2].contract_revision != conduit_text::TEXT_LITERAL_CONTRACT_REVISION
        || fixed.capabilities[3].kind != conduit_text::TEXT_UPPER_KIND
        || fixed.capabilities[3].contract_revision != conduit_text::TEXT_UPPER_CONTRACT_REVISION
        || fixed.capabilities[4].kind != conduit_std_catalog::TEXT_PRESENTATION_KIND
        || fixed.capabilities[4].contract_revision
            != conduit_std_catalog::TEXT_PRESENTATION_CONTRACT_REVISION
        || fixed.capabilities[2].implementation != crate::offer::TEXT_LITERAL_IMPLEMENTATION
        || fixed.capabilities[3].implementation != crate::offer::TEXT_UPPER_IMPLEMENTATION
        || fixed.capabilities[4].implementation != crate::offer::TEXT_PRESENTATION_IMPLEMENTATION
        || fixed.capabilities[2].required_base != crate::machine::BaseKind::Memory
        || fixed.capabilities[2].host_operation.is_some()
        || fixed.capabilities[2].maximum_output_bytes != conduit_text::MAX_TEXT_BYTES
        || fixed.capabilities[2].output.is_none_or(|port| {
            port.name != "text"
                || port.value_kind != conduit_std_catalog::TEXT_PRESENTATION_VALUE_KIND
                || port.direction != crate::offer::PortDirection::Output
        })
        || fixed.capabilities[3].required_base != crate::machine::BaseKind::Memory
        || fixed.capabilities[3].host_operation
            != Some(conduit_std_catalog::TEXT_UPPER_HOST_OPERATION_CONTRACT)
        || fixed.capabilities[3].maximum_input_bytes != conduit_text::MAX_TEXT_BYTES
        || fixed.capabilities[3].maximum_output_bytes != conduit_text::MAX_TEXT_BYTES
        || fixed.capabilities[4].required_base != crate::machine::BaseKind::Serial
        || fixed.capabilities[4].host_operation != Some("conduit.host/present@1")
        || fixed.capabilities[4].maximum_input_bytes != crate::offer::SERIAL_MAXIMUM_BYTES
        || fixed.capabilities[4].input.is_none_or(|port| {
            port.name != "text"
                || port.value_kind != conduit_std_catalog::TEXT_PRESENTATION_VALUE_KIND
                || port.direction != crate::offer::PortDirection::Input
        })
        || fixed
            .capabilities
            .iter()
            .any(|capability| capability.artifact_build != build_id)
    {
        return Err(PreparationError::OfferMismatch);
    }
    let mut literal = conduit_std_catalog::text_literal_offer();
    bind_native_capability(
        &mut literal,
        &fixed.capabilities[2],
        build_id,
        "text-literal",
    );
    let mut upper = conduit_std_catalog::text_upper_offer();
    bind_native_capability(&mut upper, &fixed.capabilities[3], build_id, "text-upper");
    let mut presentation = conduit_std_catalog::text_presentation_offer();
    bind_native_capability(
        &mut presentation,
        &fixed.capabilities[4],
        build_id,
        "presentation-text",
    );
    let mut advertisement = HostAdvertisement {
        protocol_version: PROTOCOL_VERSION,
        host_id: HostId::from(hex_identity(&identities.host)),
        boot_id: BootId::from(hex_identity(&identities.boot)),
        offer_generation: OfferGeneration(fixed.generation),
        profile: HostProfileId::from(fixed.profile),
        resources: fixed
            .resources
            .iter()
            .enumerate()
            .map(|(index, resource)| {
                resource_offer(
                    &format!("conduitos-pool-{index}-{}", resource.base.as_str()),
                    resource.class,
                    resource.capacity,
                )
            })
            .collect::<Vec<ResourceOffer>>(),
        capabilities: vec![literal, upper, presentation],
        planner_capabilities: Vec::new(),
    };
    if let Some(keyboard) = fixed.keyboard {
        crate::keyboard_offer::append_to_advertisement(&mut advertisement, keyboard, build_id)
            .map_err(|_| PreparationError::OfferMismatch)?;
    }
    #[cfg(target_arch = "x86_64")]
    if let Some(pc_speaker) = fixed.pc_speaker {
        crate::pc_speaker_offer::append_to_advertisement(&mut advertisement, pc_speaker, build_id)
            .map_err(|_| PreparationError::OfferMismatch)?;
    }
    Ok(advertisement)
}

fn validate_text_capacity(
    form: &conduit_form::ExpandedCanonicalForm,
    cord_bytes: u32,
) -> Result<(), PreparationError> {
    let literal = form
        .gears
        .iter()
        .find(|gear| gear.kind_id.as_str() == conduit_text::TEXT_LITERAL_KIND)
        .and_then(|gear| {
            gear.configuration
                .iter()
                .find_map(|entry| match (&*entry.key, &entry.value) {
                    ("value", conduit_core::ConfigurationValue::Text(value)) => Some(value),
                    _ => None,
                })
        })
        .ok_or(PreparationError::FormRejected)?;
    if literal.len() > conduit_text::MAX_TEXT_BYTES as usize || literal.len() > cord_bytes as usize
    {
        return Err(PreparationError::PlanRejected);
    }
    Ok(())
}

fn bind_native_capability(
    portable: &mut conduit_core::CapabilityOffer,
    fixed: &crate::offer::CapabilityOffer<'_>,
    build_id: &str,
    capability_name: &str,
) {
    portable.capability_id = CapabilityId::from(format!("conduitos/{capability_name}@1"));
    portable.implementation.execution_profile_id =
        ExecutionProfileId::from("conduitos/single-lane-cooperative@1");
    portable.implementation.implementation_id = ImplementationId::from(fixed.implementation);
    portable.implementation.artifact_id = ArtifactId::from(format!("conduitos-build/{build_id}"));
    portable
        .resource_requirements
        .push(conduit_core::resource_requirement(
            "conduit.resource/runtime-memory@1",
            4_096,
        ));
    portable.resource_requirements.sort();
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::offer::CpuFeatures;
    use conduit_core::{ExecutionScheduling, FormIdentity, seal_plan};

    fn fixture() -> (BootIdentities, HostOffer<'static>) {
        let identities = BootIdentities {
            host: [1; 32],
            boot: [2; 32],
        };
        let offer = HostOffer::new(
            &identities,
            "build",
            CpuFeatures {
                sse2: true,
                rdrand: true,
                invariant_tsc: true,
            },
            256 * 1024,
        );
        (identities, offer)
    }

    #[test]
    fn ordinary_source_checks_plans_lowers_and_installs() {
        let (identities, offer) = fixture();
        let prepared = prepare(&identities, &offer, "build").unwrap();
        assert_eq!(prepared.active_play.plan_id, prepared.plan_id);
        assert_eq!(
            prepared.active_play.boot_id.as_str(),
            hex_identity(&identities.boot)
        );
        assert!(prepared.planned_sign_items > 0 && prepared.planned_sign_bytes > 0);
        let [region] = prepared.plan.fragments[0].execution_regions.as_slice() else {
            panic!("ordinary Plan must contain exactly one execution region");
        };
        assert_eq!(region.region_id.as_str(), "region/0");
        assert_eq!(region.admitted_placements.len(), ORDINARY_PLACEMENT_COUNT);
        assert_eq!(
            region.execution_profile_id.as_str(),
            COOPERATIVE_REGION_PROFILE
        );
        assert_eq!(
            region.scheduling,
            ExecutionScheduling::CooperativeBoundedStep
        );
        assert_eq!(region.lane_count, 1);
        assert_eq!(region.lane_resource.units, 1);
        assert_eq!(region.requirements.runtime_memory_bytes, 12_288);
        assert_eq!(region.requirements.timer_slots, 0);
        assert_eq!(region.requirements.cord_item_capacity, 2);
        assert_eq!(region.requirements.cord_byte_capacity, CORD_BYTES * 2);
        assert!(!region.preemption_required && !region.isolation_required);
        assert!(!ORDINARY_FORM_SOURCE.contains("lane"));
        assert!(!ORDINARY_FORM_SOURCE.contains("preemption"));
    }

    #[test]
    fn resealed_wrong_lane_requirement_is_rejected_before_play() {
        let (identities, offer) = fixture();
        let prepared = prepare(&identities, &offer, "build").unwrap();
        let mut fragments = prepared.plan.fragments;
        fragments[0].execution_regions[0].lane_count = 2;
        fragments[0].execution_regions[0].lane_resource.units = 2;
        fragments[0].execution_regions[0]
            .lane_resource
            .compute
            .as_mut()
            .unwrap()
            .selected_lanes = 2;
        let plan = seal_plan(
            FormIdentity {
                source_document_id: prepared.source_document_id,
                checked_form_id: prepared.checked_form_id,
                expanded_form_id: prepared.expanded_form_id,
            },
            fragments,
        );
        assert!(conduit_core::verify_plan(&plan));
        assert_eq!(
            validate_execution_region(&plan.fragments[0], &prepared.advertisement, &offer),
            Err(PreparationError::PlanRejected)
        );
    }

    #[test]
    fn unavailable_execution_lane_is_rejected_before_play() {
        let (identities, mut offer) = fixture();
        let lane = offer
            .bases
            .iter_mut()
            .find(|base| base.kind == crate::machine::BaseKind::ExecutionLane)
            .unwrap();
        lane.capacity = 0;
        assert_eq!(
            prepare(&identities, &offer, "build").err(),
            Some(PreparationError::OfferMismatch)
        );
    }

    #[test]
    fn stale_boot_and_unavailable_implementation_fail_closed() {
        let (identities, mut offer) = fixture();
        offer.boot_id = [3; 32];
        assert!(matches!(
            prepare(&identities, &offer, "build"),
            Err(PreparationError::OfferMismatch)
        ));
        offer.boot_id = identities.boot;
        offer.capabilities[2].maximum_output_bytes = (TEXT_LITERAL.len() - 1) as u32;
        assert_eq!(
            prepare(&identities, &offer, "build").err(),
            Some(PreparationError::OfferMismatch)
        );
        offer.capabilities[2].maximum_output_bytes = conduit_text::MAX_TEXT_BYTES;
        offer.capabilities[3].implementation = "unavailable";
        assert_eq!(
            prepare(&identities, &offer, "build").err(),
            Some(PreparationError::OfferMismatch)
        );
        offer.capabilities[3].implementation = crate::offer::TEXT_UPPER_IMPLEMENTATION;
        offer.capabilities[3].maximum_output_bytes -= 1;
        assert_eq!(
            prepare(&identities, &offer, "build").err(),
            Some(PreparationError::OfferMismatch)
        );
    }

    #[test]
    fn stale_offer_plan_identity_and_missing_serial_base_fail_closed() {
        let (identities, mut offer) = fixture();
        let prepared = prepare(&identities, &offer, "build").unwrap();

        offer.generation += 1;
        assert_eq!(
            validate_execution_region(&prepared.plan.fragments[0], &prepared.advertisement, &offer,),
            Err(PreparationError::PlanRejected)
        );

        let mut stale_plan = prepared.plan;
        stale_plan.plan_id = PlanId::from("stale-plan");
        assert!(!conduit_core::verify_plan(&stale_plan));

        let (identities, mut offer) = fixture();
        offer
            .bases
            .iter_mut()
            .find(|base| base.kind == crate::machine::BaseKind::Serial)
            .unwrap()
            .capacity = 0;
        assert_eq!(
            prepare(&identities, &offer, "build").err(),
            Some(PreparationError::OfferMismatch)
        );
    }

    #[test]
    fn insufficient_memory_timer_and_sign_reserves_fail_before_play() {
        let (identities, mut offer) = fixture();
        offer.resources[0].capacity = 4_096;
        assert!(matches!(
            prepare(&identities, &offer, "build"),
            Err(PreparationError::PlanRejected)
        ));
        offer.resources[0].capacity = 256 * 1024;
        offer.resources[2].capacity = 0;
        assert!(matches!(
            prepare(&identities, &offer, "build"),
            Err(PreparationError::OfferMismatch)
        ));
        offer.resources[2].capacity = 1;
        offer.sign_item_capacity = 6;
        assert!(matches!(
            prepare(&identities, &offer, "build"),
            Err(PreparationError::PlanRejected)
        ));
    }

    #[test]
    fn undersized_cord_reserve_and_stale_planned_boot_fail_closed() {
        let (identities, offer) = fixture();
        let advertisement = advertisement(&identities, &offer, "build").unwrap();
        let form = crate::ordinary_form::checked_expanded_text_form(ORDINARY_FORM_SOURCE).unwrap();
        let hosts = [advertisement];
        let placements = default_expanded_placements(&form, &hosts).unwrap();
        assert_eq!(
            validate_text_capacity(&form, (TEXT_LITERAL.len() - 1) as u32),
            Err(PreparationError::PlanRejected)
        );
        let mut plan = conduit_planner::plan_expanded_canonical(
            &form,
            &hosts,
            &placements,
            &[BaseImplementationId::from("conduit.base/local@1")],
        )
        .unwrap();
        plan.fragments[0].boot_id = BootId::from("stale-boot");
        assert!(lower_plan_fragment(&plan.fragments[0]).is_err());
    }

    #[test]
    fn oversized_text_is_refused_during_source_checking() {
        let oversized = "x".repeat(conduit_text::MAX_TEXT_BYTES as usize + 1);
        let source = format!("form too-large {{\n    \"{oversized}\" > presentation/text\n}}\n");
        let syntax = conduit_form::parse_syntax_document(&source);
        let mut startup = conduit_form::StartupCatalog::new();
        let mut profile = conduit_form::ProfileCatalog::new();
        conduit_std_catalog::install_text_pipeline_catalogs(&mut startup, &mut profile).unwrap();
        let checked = conduit_form::check_syntax_document(&syntax, &startup).unwrap();
        assert!(conduit_form::expand_canonical_form(&checked, "too-large", &profile).is_err());

        let malformed = conduit_form::parse_syntax_document(
            "form malformed {\n    \"bad\\q\" > presentation/text\n}\n",
        );
        assert!(conduit_form::check_syntax_document(&malformed, &startup).is_err());
    }
}
