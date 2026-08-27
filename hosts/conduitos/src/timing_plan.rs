//! Exact planning for the accepted tick-only timing proof.

use alloc::{format, string::String, vec, vec::Vec};
use core::fmt::Write;

use conduit_core::{
    ActivePlayIdentity, ArtifactId, BaseImplementationId, BootId, CapabilityId, ExecutionProfileId,
    HostAdvertisement, HostId, HostProfileId, ImplementationId, OfferGeneration, PROTOCOL_VERSION,
    Plan, PlanId, ResourceOffer, bind_active_play, resource_offer,
};
use conduit_plan_lowering::lowering::lower_plan_fragment;
use conduit_planner::{PlanningOptions, default_placements, plan_with_options};

use crate::{
    execution_region::{seal_execution_region, validate_execution_region},
    identity::BootIdentities,
    offer::HostOffer,
    planned_kernel::PlannedKernel,
};

const TIMING_FORM_SOURCE: &str = "form conduitos-ordinary {\n    clock: time/tick(count = 1, period-ms = 1)\n    show: presentation/tick(maximum-values = 1)\n\n\n    clock.tick > show.tick\n}\n";

pub struct PreparedTimingPlay {
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

pub fn prepare_timing(
    identities: &BootIdentities,
    fixed_offer: &HostOffer<'_>,
    build_id: &str,
) -> Result<PreparedTimingPlay, PreparationError> {
    let advertisement = advertisement(identities, fixed_offer, build_id)?;
    let mut catalog = conduit_form::ProfileCatalog::new();
    catalog
        .insert(conduit_time::tick_kind_definition())
        .map_err(|_| PreparationError::FormRejected)?;
    catalog
        .insert(conduit_std_catalog::tick_presentation_kind_definition())
        .map_err(|_| PreparationError::FormRejected)?;
    let form = conduit_form::parse(TIMING_FORM_SOURCE, &catalog)
        .map_err(|_| PreparationError::FormRejected)?;
    let hosts = [advertisement.clone()];
    let placements =
        default_placements(&form, &hosts).map_err(|_| PreparationError::PlacementRejected)?;
    let plan = plan_with_options(
        &form,
        &hosts,
        &placements,
        &[BaseImplementationId::from("conduit.base/local@1")],
        PlanningOptions {
            connection_bases: &alloc::collections::BTreeMap::new(),
            line_candidates: &alloc::collections::BTreeMap::new(),
            connection_item_capacity: 1,
            connection_byte_capacity: conduit_time::TICK_ENCODED_LEN,
            authority_grants: &[],
            protected_resource_grants: &[],
            line_offers: &[],
        },
    )
    .map_err(|_| PreparationError::PlanRejected)?;
    let plan = seal_execution_region(plan, &advertisement, fixed_offer)
        .map_err(|_| PreparationError::PlanRejected)?;
    if !conduit_core::verify_plan(&plan) || plan.fragments.len() != 1 {
        return Err(PreparationError::PlanRejected);
    }
    let fragment = &plan.fragments[0];
    validate_execution_region(fragment, &advertisement, fixed_offer)
        .map_err(|_| PreparationError::PlanRejected)?;
    if fragment.host_id != hosts[0].host_id
        || fragment.boot_id != hosts[0].boot_id
        || fragment.offer_generation != hosts[0].offer_generation
        || fragment.placements.len() != 2
        || fragment.placements.iter().any(|placement| {
            placement.host_id != hosts[0].host_id || placement.boot_id != hosts[0].boot_id
        })
    {
        return Err(PreparationError::PlanRejected);
    }
    let lowered = lower_plan_fragment(fragment).map_err(|_| PreparationError::LoweringRejected)?;
    if lowered.sign_items > fixed_offer.sign_item_capacity
        || lowered.cord_value_slots > 1
        || lowered.cord_value_bytes > conduit_time::TICK_ENCODED_LEN
    {
        return Err(PreparationError::PlanRejected);
    }
    let kernel =
        PlannedKernel::prepare(fragment, &lowered).map_err(|_| PreparationError::KernelRejected)?;
    let active_play = bind_active_play(&plan.plan_id, &fragment.host_id, &fragment.boot_id, 0);
    Ok(PreparedTimingPlay {
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
        || fixed.capabilities.len() != crate::offer::CAPABILITY_COUNT
        || fixed.capabilities[0].kind != conduit_std_catalog::TICK_KIND
        || fixed.capabilities[0].contract_revision != conduit_time::TICK_CONTRACT_REVISION
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
    let mut tick = crate::functional_offers::tick_offer();
    bind_native_capability(&mut tick, &fixed.capabilities[0], build_id, "time-tick");
    let mut presentation = crate::presentation_nucleus::presentation_offer_for(
        conduit_std_catalog::TICK_PRESENTATION_KIND,
    )
    .expect("ConduitOS owns tick presentation");
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
