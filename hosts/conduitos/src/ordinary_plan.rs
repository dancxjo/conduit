//! Current Form check, exact boot-scoped planning, and numeric lowering.

use alloc::{format, string::String, vec, vec::Vec};
use core::fmt::Write;

use conduit_core::{
    ActivePlayIdentity, ArtifactId, BootId, CapabilityId, ConnectionBase, ExecutionProfileId,
    HostAdvertisement, HostId, HostProfileId, ImplementationId, OfferGeneration, PROTOCOL_VERSION,
    Plan, PlanId, ResourceOffer, bind_active_play, resource_offer,
};
use conduit_planner::{PlanningOptions, default_placements, plan_with_options};
use conduit_runtime::lowering::lower_plan_fragment;

use crate::{
    execution_region::{seal_execution_region, validate_execution_region},
    identity::BootIdentities,
    offer::{CAPABILITY_COUNT, HostOffer},
    planned_kernel::PlannedKernel,
};

pub const ORDINARY_FORM_SOURCE: &str = "form 0\n\nconduitos-ordinary {\n    clock: time/tick\n    show: presentation/tick\n\n    clock.count = 1\n    clock.period-ms = 1\n    show.maximum-values = 1\n\n    clock.tick -> show.tick\n}\n";
pub const COOPERATIVE_REGION_PROFILE: &str = "conduitos/cooperative-bounded-step@1";

pub struct PreparedOrdinaryPlay {
    pub kernel: PlannedKernel,
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
    let mut catalog = conduit_std_catalog::tick_profile_catalog();
    catalog
        .insert(conduit_std_catalog::tick_presentation_kind_definition())
        .map_err(|_| PreparationError::FormRejected)?;
    let form = conduit_form::parse(ORDINARY_FORM_SOURCE, &catalog)
        .map_err(|_| PreparationError::FormRejected)?;
    let hosts = [advertisement.clone()];
    let placements =
        default_placements(&form, &hosts).map_err(|_| PreparationError::PlacementRejected)?;
    let plan = plan_with_options(
        &form,
        &hosts,
        &placements,
        &[ConnectionBase::Local],
        PlanningOptions {
            connection_bases: &alloc::collections::BTreeMap::new(),
            line_candidates: &alloc::collections::BTreeMap::new(),
            connection_item_capacity: 1,
            connection_byte_capacity: conduit_std_catalog::TICK_ENCODED_LEN,
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
        || fragment.placements.len() != CAPABILITY_COUNT
        || fragment.placements.iter().any(|placement| {
            placement.host_id != hosts[0].host_id || placement.boot_id != hosts[0].boot_id
        })
    {
        return Err(PreparationError::PlanRejected);
    }
    let lowered = lower_plan_fragment(fragment).map_err(|_| PreparationError::LoweringRejected)?;
    if lowered.sign_items > fixed_offer.sign_item_capacity
        || lowered.cord_value_slots > 1
        || lowered.cord_value_bytes > conduit_std_catalog::TICK_ENCODED_LEN
    {
        return Err(PreparationError::PlanRejected);
    }
    let kernel =
        PlannedKernel::prepare(fragment, &lowered).map_err(|_| PreparationError::KernelRejected)?;
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

fn advertisement(
    identities: &BootIdentities,
    fixed: &HostOffer<'_>,
    build_id: &str,
) -> Result<HostAdvertisement, PreparationError> {
    if fixed.host_id != identities.host
        || fixed.boot_id != identities.boot
        || fixed.generation == 0
        || fixed.capabilities.len() != CAPABILITY_COUNT
        || fixed.capabilities[0].kind != conduit_std_catalog::TICK_KIND
        || fixed.capabilities[0].contract_revision != conduit_std_catalog::TICK_CONTRACT_REVISION
        || fixed.capabilities[1].kind != conduit_std_catalog::TICK_PRESENTATION_KIND
        || fixed.capabilities[1].contract_revision
            != conduit_std_catalog::TICK_PRESENTATION_CONTRACT_REVISION
        || fixed.capabilities[0].implementation != crate::offer::TIME_TICK_IMPLEMENTATION
        || fixed.capabilities[1].implementation != crate::offer::TICK_PRESENTATION_IMPLEMENTATION
        || fixed
            .capabilities
            .iter()
            .any(|capability| capability.artifact_build != build_id)
    {
        return Err(PreparationError::OfferMismatch);
    }
    let mut tick = conduit_std_catalog::tick_capability_offer();
    bind_native_capability(&mut tick, &fixed.capabilities[0], build_id, "time-tick");
    let mut presentation = conduit_std_catalog::tick_presentation_offer();
    bind_native_capability(
        &mut presentation,
        &fixed.capabilities[1],
        build_id,
        "presentation-tick",
    );
    Ok(HostAdvertisement {
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
        capabilities: vec![tick, presentation],
        planner_capabilities: Vec::new(),
    })
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

fn hex_identity(bytes: &[u8; 32]) -> String {
    let mut result = String::with_capacity(64);
    for byte in bytes {
        write!(&mut result, "{byte:02x}").expect("writing to String cannot fail");
    }
    result
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
        assert_eq!(region.admitted_placements.len(), CAPABILITY_COUNT);
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
        assert_eq!(region.requirements.runtime_memory_bytes, 8_192);
        assert_eq!(region.requirements.timer_slots, 1);
        assert_eq!(region.requirements.cord_item_capacity, 1);
        assert_eq!(
            region.requirements.cord_byte_capacity,
            conduit_std_catalog::TICK_ENCODED_LEN
        );
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
            Some(PreparationError::PlanRejected)
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
        offer.capabilities[0].implementation = "unavailable";
        assert!(matches!(
            prepare(&identities, &offer, "build"),
            Err(PreparationError::OfferMismatch)
        ));
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
            Err(PreparationError::PlanRejected)
        ));
        offer.resources[2].capacity = 1;
        offer.sign_item_capacity = 6;
        assert!(matches!(
            prepare(&identities, &offer, "build"),
            Err(PreparationError::PlanRejected)
        ));
    }

    #[test]
    fn zero_cord_reserve_and_stale_planned_boot_fail_closed() {
        let (identities, offer) = fixture();
        let advertisement = advertisement(&identities, &offer, "build").unwrap();
        let mut catalog = conduit_std_catalog::tick_profile_catalog();
        catalog
            .insert(conduit_std_catalog::tick_presentation_kind_definition())
            .unwrap();
        let form = conduit_form::parse(ORDINARY_FORM_SOURCE, &catalog).unwrap();
        let hosts = [advertisement];
        let placements = default_placements(&form, &hosts).unwrap();
        let empty_bases = alloc::collections::BTreeMap::new();
        let empty_candidates = alloc::collections::BTreeMap::new();
        assert!(
            plan_with_options(
                &form,
                &hosts,
                &placements,
                &[ConnectionBase::Local],
                PlanningOptions {
                    connection_bases: &empty_bases,
                    line_candidates: &empty_candidates,
                    connection_item_capacity: 0,
                    connection_byte_capacity: 0,
                    authority_grants: &[],
                    protected_resource_grants: &[],
                    line_offers: &[],
                },
            )
            .is_err()
        );
        let mut plan =
            conduit_planner::plan(&form, &hosts, &placements, &[ConnectionBase::Local]).unwrap();
        plan.fragments[0].boot_id = BootId::from("stale-boot");
        assert!(lower_plan_fragment(&plan.fragments[0]).is_err());
    }
}
