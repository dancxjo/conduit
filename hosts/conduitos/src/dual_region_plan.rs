//! Ordinary planning of one portable Form with two independent branches.

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
    dual_region_kernel::DualRegionKernel,
    execution_region::{seal_two_execution_regions, validate_two_execution_regions},
    identity::{BootIdentities, hex as hex_identity},
    offer::{CAPABILITY_COUNT, HostOffer},
    ordinary_plan::PreparationError,
};

pub const FORM_SOURCE: &str = "form conduitos-two-regions {\n    clock: time/tick(count = 1, period-ms = 1)\n    ticks: presentation/tick(maximum-values = 1)\n    upper: text/upper\n    text: presentation/text\n    clock > ticks\n    \"Hello, ConduitOS\" > upper > text\n}\n";
pub const TEXT_RESULT: &str = "HELLO, CONDUITOS";
const PLACEMENT_COUNT: usize = 5;
const CORD_COUNT: usize = 3;
const CORD_BYTES: u32 = 64;

pub struct PreparedDualRegionPlay {
    pub kernel: DualRegionKernel,
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

pub fn prepare(
    identities: &BootIdentities,
    fixed_offer: &HostOffer<'_>,
    build_id: &str,
) -> Result<PreparedDualRegionPlay, PreparationError> {
    let advertisement = advertisement(identities, fixed_offer, build_id)?;
    stage(b"advertisement");
    let form = checked_expanded_form()?;
    stage(b"form");
    let hosts = [advertisement.clone()];
    let placements = default_expanded_placements(&form, &hosts)
        .map_err(|_| PreparationError::PlacementRejected)?;
    stage(b"placements");
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
    stage(b"planned");
    let plan = seal_two_execution_regions(plan, &advertisement, fixed_offer)?;
    stage(b"regions");
    if !conduit_core::verify_plan(&plan) || plan.fragments.len() != 1 {
        return Err(PreparationError::PlanRejected);
    }
    let fragment = &plan.fragments[0];
    validate_two_execution_regions(fragment, &advertisement, fixed_offer)?;
    if fragment.host_id != hosts[0].host_id
        || fragment.boot_id != hosts[0].boot_id
        || fragment.offer_generation != hosts[0].offer_generation
        || fragment.placements.len() != PLACEMENT_COUNT
        || fragment.connections.len() != CORD_COUNT
    {
        return Err(PreparationError::PlanRejected);
    }
    let lowered = lower_plan_fragment(fragment).map_err(|_| PreparationError::LoweringRejected)?;
    stage(b"lowered");
    if lowered.sign_items > fixed_offer.sign_item_capacity
        || lowered.cord_value_slots != CORD_COUNT as u16
        || lowered.cord_value_bytes != CORD_BYTES * CORD_COUNT as u32
    {
        return Err(PreparationError::PlanRejected);
    }
    let kernel = DualRegionKernel::prepare(fragment, &lowered)
        .map_err(|_| PreparationError::KernelRejected)?;
    stage(b"kernel");
    let active_play = bind_active_play(&plan.plan_id, &fragment.host_id, &fragment.boot_id, 0);
    Ok(PreparedDualRegionPlay {
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

#[cfg(all(target_os = "none", target_arch = "x86_64"))]
fn stage(name: &[u8]) {
    crate::arch::early_write(b"CONDUIT_PLAN_STAGE ");
    crate::arch::early_write(name);
    crate::arch::early_write(b"\n");
}

#[cfg(all(target_os = "none", target_arch = "arm"))]
fn stage(name: &[u8]) {
    crate::arch::present(b"CONDUIT_PLAN_STAGE ");
    crate::arch::present(name);
    crate::arch::present(b"\n");
}

#[cfg(not(any(
    all(target_os = "none", target_arch = "x86_64"),
    all(target_os = "none", target_arch = "arm")
)))]
fn stage(_name: &[u8]) {}

fn checked_expanded_form() -> Result<conduit_form::ExpandedCanonicalForm, PreparationError> {
    let syntax = conduit_form::parse_syntax_document(FORM_SOURCE);
    stage(b"syntax");
    let mut startup = conduit_form::StartupCatalog::new();
    let mut profile = conduit_form::ProfileCatalog::new();
    conduit_std_catalog::install_text_pipeline_catalogs(&mut startup, &mut profile)
        .map_err(|_| PreparationError::FormRejected)?;
    conduit_time::install_tick_catalog(&mut startup, &mut profile)
        .map_err(|_| PreparationError::FormRejected)?;
    conduit_std_catalog::install_tick_presentation_catalog(&mut startup, &mut profile)
        .map_err(|_| PreparationError::FormRejected)?;
    stage(b"catalogs");
    let checked = conduit_form::check_syntax_document(&syntax, &startup)
        .map_err(|_| PreparationError::FormRejected)?;
    stage(b"checked");
    let expanded = conduit_form::expand_canonical_form(&checked, "conduitos-two-regions", &profile)
        .map_err(|_| PreparationError::FormRejected)?;
    stage(b"expanded");
    Ok(expanded)
}

fn advertisement(
    identities: &BootIdentities,
    fixed: &HostOffer<'_>,
    build_id: &str,
) -> Result<HostAdvertisement, PreparationError> {
    fixed
        .validate()
        .map_err(|_| PreparationError::OfferMismatch)?;
    if fixed.host_id != identities.host
        || fixed.boot_id != identities.boot
        || fixed.capabilities.len() != CAPABILITY_COUNT
        || fixed
            .capabilities
            .iter()
            .any(|capability| capability.artifact_build != build_id)
    {
        return Err(PreparationError::OfferMismatch);
    }
    let mut capabilities = vec![
        crate::functional_offers::tick_offer(),
        crate::presentation_offers::presentation_offer_for(
            conduit_std_catalog::TICK_PRESENTATION_KIND,
        )
        .expect("ConduitOS owns tick presentation"),
        conduit_std_catalog::text_literal_offer(),
        conduit_std_catalog::text_upper_offer(),
        crate::presentation_offers::presentation_offer_for(
            conduit_std_catalog::TEXT_PRESENTATION_KIND,
        )
        .expect("ConduitOS owns text presentation"),
    ];
    for (index, capability) in capabilities.iter_mut().enumerate() {
        let fixed_capability = &fixed.capabilities[index];
        if capability.kind_id.as_str() != fixed_capability.kind
            || capability.kind_contract_revision.as_str() != fixed_capability.contract_revision
        {
            return Err(PreparationError::OfferMismatch);
        }
        bind_native_capability(capability, fixed_capability, build_id, index);
    }
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
        capabilities,
        planner_capabilities: Vec::new(),
    };
    if let Some(keyboard) = fixed.keyboard {
        crate::keyboard_offer::append_to_advertisement(&mut advertisement, keyboard, build_id)
            .map_err(|_| PreparationError::OfferMismatch)?;
    }
    Ok(advertisement)
}

fn bind_native_capability(
    portable: &mut conduit_core::CapabilityOffer,
    fixed: &crate::offer::CapabilityOffer<'_>,
    build_id: &str,
    index: usize,
) {
    portable.capability_id = CapabilityId::from(format!("conduitos/dual-{index}@1"));
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
    use conduit_core::{FormIdentity, HostBaseId, ResourcePoolId, seal_plan};

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
            512 * 1024,
        );
        (identities, offer)
    }

    #[test]
    fn unchanged_form_plans_two_disjoint_exact_regions() {
        let (identities, offer) = fixture();
        let prepared = prepare(&identities, &offer, "build").unwrap();
        let regions = &prepared.plan.fragments[0].execution_regions;
        assert_eq!(regions.len(), 2);
        assert_eq!(regions[0].admitted_placements.len(), 3);
        assert_eq!(regions[1].admitted_placements.len(), 2);
        assert_ne!(
            regions[0].lane_resource.pool_id,
            regions[1].lane_resource.pool_id
        );
        assert_eq!(regions[0].lane_base_id, regions[1].lane_base_id);
        assert!(regions.iter().all(|region| {
            region.execution_profile_id.as_str() == crate::ordinary_plan::COOPERATIVE_REGION_PROFILE
                && region.lane_count == 1
                && !region.preemption_required
                && !region.isolation_required
        }));
        assert!(!FORM_SOURCE.contains("lane"));
        assert!(!FORM_SOURCE.contains("thread"));
        assert!(!FORM_SOURCE.contains("scheduler"));
    }

    fn reseal(
        prepared: &PreparedDualRegionPlay,
        fragments: Vec<conduit_core::PlanFragment>,
    ) -> Plan {
        seal_plan(
            FormIdentity {
                source_document_id: prepared.source_document_id.clone(),
                checked_form_id: prepared.checked_form_id.clone(),
                expanded_form_id: prepared.expanded_form_id.clone(),
            },
            fragments,
        )
    }

    #[test]
    fn every_region_realization_fact_is_sealed_and_revalidated() {
        let (identities, offer) = fixture();
        let prepared = prepare(&identities, &offer, "build").unwrap();
        let original_id = prepared.plan_id.clone();

        let mut mutations = Vec::new();
        let mut membership = prepared.plan.fragments.clone();
        membership[0].execution_regions[0].admitted_placements.pop();
        mutations.push(membership);

        let mut duplicate_lane = prepared.plan.fragments.clone();
        duplicate_lane[0].execution_regions[1].lane_resource =
            duplicate_lane[0].execution_regions[0].lane_resource.clone();
        mutations.push(duplicate_lane);

        let mut capacity = prepared.plan.fragments.clone();
        capacity[0].execution_regions[0].lane_resource.units = 2;
        capacity[0].execution_regions[0].lane_count = 2;
        capacity[0].execution_regions[0]
            .lane_resource
            .compute
            .as_mut()
            .unwrap()
            .selected_lanes = 2;
        mutations.push(capacity);

        let mut stale_pool = prepared.plan.fragments.clone();
        stale_pool[0].execution_regions[0].lane_resource.pool_id =
            ResourcePoolId::from("stale-lane");
        mutations.push(stale_pool);

        let mut stale_base = prepared.plan.fragments.clone();
        stale_base[0].execution_regions[0].lane_base_id = HostBaseId::from("stale-base");
        stale_base[0].execution_regions[0]
            .lane_resource
            .compute
            .as_mut()
            .unwrap()
            .architecture_base_id = conduit_core::ArchitectureBaseId::from("stale-base");
        mutations.push(stale_base);

        let mut budget = prepared.plan.fragments.clone();
        budget[0].execution_regions[0]
            .requirements
            .runtime_memory_bytes += 1;
        mutations.push(budget);

        let mut one_lane_lie = prepared.plan.fragments.clone();
        one_lane_lie[0].execution_regions.pop();
        mutations.push(one_lane_lie);

        for fragments in mutations {
            let changed = reseal(&prepared, fragments);
            assert_ne!(changed.plan_id, original_id);
            assert_eq!(
                validate_two_execution_regions(
                    &changed.fragments[0],
                    &prepared.advertisement,
                    &offer
                ),
                Err(PreparationError::PlanRejected)
            );
        }
    }

    #[test]
    fn unavailable_second_lane_and_sign_storage_refuse_before_play() {
        let (identities, mut offer) = fixture();
        offer.resources[4].capacity = 0;
        assert!(matches!(
            prepare(&identities, &offer, "build"),
            Err(PreparationError::OfferMismatch)
        ));

        offer.resources[4].capacity = 1;
        offer.sign_item_capacity = 1;
        assert!(matches!(
            prepare(&identities, &offer, "build"),
            Err(PreparationError::PlanRejected)
        ));
    }

    #[test]
    fn cancellation_is_one_bounded_terminal_kernel_state() {
        let (identities, offer) = fixture();
        let mut prepared = prepare(&identities, &offer, "build").unwrap();
        prepared.kernel.cancel().unwrap();
        assert_eq!(
            prepared.kernel.step().unwrap(),
            conduit_kernel::scheduler::SchedulerStatus::Cancelled
        );
        assert!(prepared.kernel.sign_count() <= 96);
    }
}
