//! Current plot check, exact boot-scoped planning, and numeric lowering.

use alloc::{format, vec, vec::Vec};
use core::sync::atomic::{AtomicU32, Ordering};
static NEXT_PLAY: AtomicU32 = AtomicU32::new(1);

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

pub const ORDINARY_PLOT_SOURCE: &str = "plot conduitos-text-upper {\n    upper: text/upper\n    show: presentation/text\n    \"Hello, ConduitOS\" >> upper >> show\n}\n";
pub const TEXT_LITERAL: &str = "Hello, ConduitOS";
pub const TEXT_RESULT: &str = "HELLO, CONDUITOS";
const CORD_BYTES: u32 = conduit_text::MAX_TEXT_BYTES;
const ORDINARY_PLACEMENT_COUNT: usize = 3;
pub const COOPERATIVE_REGION_PROFILE: &str = "conduitos/cooperative-bounded-step@1";
pub const PROTECTED_REGION_PROFILE: &str = "conduitos/protected-region@1";

pub(crate) const fn region_profile(protected: bool) -> &'static str {
    if protected {
        PROTECTED_REGION_PROFILE
    } else {
        COOPERATIVE_REGION_PROFILE
    }
}

pub(crate) fn new_play(
    plan: &PlanId,
    host: &HostId,
    boot: &BootId,
) -> Result<ActivePlayIdentity, PreparationError> {
    let sequence = NEXT_PLAY
        .fetch_update(Ordering::AcqRel, Ordering::Acquire, |value| {
            value.checked_add(1)
        })
        .map_err(|_| PreparationError::PlayIdentityExhausted)?;
    Ok(bind_active_play(plan, host, boot, u64::from(sequence)))
}

pub struct PreparedOrdinaryPlay {
    pub kernel: TextPlannedKernel,
    pub advertisement: HostAdvertisement,
    pub plan: Plan,
    pub source_document_id: conduit_core::SourceDocumentId,
    pub checked_plot_id: conduit_core::CheckedPlotId,
    pub expanded_plot_id: conduit_core::ExpandedPlotId,
    pub plan_id: PlanId,
    pub fragment_id: conduit_core::FragmentId,
    pub active_play: ActivePlayIdentity,
    pub planned_sign_items: u16,
    pub planned_sign_bytes: u32,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PreparationError {
    OfferMismatch,
    PlotRejected,
    PlacementRejected,
    PlanRejected,
    LoweringRejected,
    KernelRejected,
    Protection(crate::protected_region::DomainRefusal),
    PlayIdentityExhausted,
}

impl PreparationError {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::OfferMismatch => "ordinary-offer-mismatch",
            Self::PlotRejected => "ordinary-plot-rejected",
            Self::PlacementRejected => "ordinary-placement-rejected",
            Self::PlanRejected => "ordinary-plan-rejected",
            Self::LoweringRejected => "ordinary-lowering-rejected",
            Self::KernelRejected => "ordinary-kernel-rejected",
            Self::Protection(refusal) => refusal.as_str(),
            Self::PlayIdentityExhausted => "ordinary-play-identity-capacity-exhausted",
        }
    }
}

pub fn prepare(
    identities: &BootIdentities,
    fixed_offer: &HostOffer<'_>,
    build_id: &str,
) -> Result<PreparedOrdinaryPlay, PreparationError> {
    prepare_source(
        identities,
        fixed_offer,
        build_id,
        ORDINARY_PLOT_SOURCE,
        "conduitos-text-upper",
        TEXT_LITERAL,
    )
}

pub fn prepare_source(
    identities: &BootIdentities,
    fixed_offer: &HostOffer<'_>,
    build_id: &str,
    source: &str,
    plot_name: &str,
    expected_literal: &str,
) -> Result<PreparedOrdinaryPlay, PreparationError> {
    let advertisement = advertisement(identities, fixed_offer, build_id)?;
    let plot = crate::ordinary_plot::checked_expanded_text_plot_named(source, plot_name)?;
    validate_text_capacity(&plot, CORD_BYTES)?;
    let hosts = [advertisement.clone()];
    let placements = default_expanded_placements(&plot, &hosts)
        .map_err(|_| PreparationError::PlacementRejected)?;
    let plan = plan_expanded_canonical_with_options(
        &plot,
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
    #[allow(unused_mut)]
    let mut kernel = TextPlannedKernel::prepare_with_literal(fragment, &lowered, expected_literal)
        .map_err(|_| PreparationError::KernelRejected)?;
    let active_play = new_play(&plan.plan_id, &fragment.host_id, &fragment.boot_id)?;
    #[cfg(conduitos_protected_execution)]
    kernel
        .protect(&plan, &active_play, fixed_offer)
        .map_err(|error| match error {
            crate::composition::MachineRunError::ProtectionDomain(refusal) => {
                PreparationError::Protection(refusal)
            }
            _ => PreparationError::KernelRejected,
        })?;
    Ok(PreparedOrdinaryPlay {
        kernel,
        advertisement,
        source_document_id: plan.source_document_id.clone(),
        checked_plot_id: plan.checked_plot_id.clone(),
        expanded_plot_id: plan.expanded_plot_id.clone(),
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
        || fixed.capabilities[4].kind != conduit_semantic_catalog::TEXT_PRESENTATION_KIND
        || fixed.capabilities[4].contract_revision
            != conduit_semantic_catalog::TEXT_PRESENTATION_CONTRACT_REVISION
        || fixed.capabilities[2].implementation != crate::offer::TEXT_LITERAL_IMPLEMENTATION
        || fixed.capabilities[3].implementation != crate::offer::TEXT_UPPER_IMPLEMENTATION
        || fixed.capabilities[4].implementation != crate::offer::TEXT_PRESENTATION_IMPLEMENTATION
        || fixed.capabilities[5].kind != conduit_text::TEXT_MORSE_KIND
        || fixed.capabilities[5].contract_revision != conduit_text::TEXT_MORSE_CONTRACT_REVISION
        || fixed.capabilities[5].implementation != crate::offer::TEXT_MORSE_IMPLEMENTATION
        || fixed.capabilities[6].kind != conduit_semantic_catalog::INDICATOR_PRESENTATION_KIND
        || fixed.capabilities[6].contract_revision
            != conduit_semantic_catalog::INDICATOR_PRESENTATION_CONTRACT_REVISION
        || fixed.capabilities[6].implementation
            != crate::offer::INDICATOR_PRESENTATION_IMPLEMENTATION
        || [
            (
                conduit_text::TEXT_CHARACTERS_KIND,
                crate::offer::TEXT_CHARACTERS_IMPLEMENTATION,
            ),
            (
                conduit_text::MORSE_LOOKUP_KIND,
                crate::offer::MORSE_LOOKUP_IMPLEMENTATION,
            ),
            (
                conduit_text::MORSE_INTERSPERSE_KIND,
                crate::offer::MORSE_INTERSPERSE_IMPLEMENTATION,
            ),
            (
                conduit_text::MORSE_FLATTEN_KIND,
                crate::offer::MORSE_FLATTEN_IMPLEMENTATION,
            ),
            (
                conduit_text::MORSE_SYMBOLS_TO_PATTERN_KIND,
                crate::offer::MORSE_SYMBOLS_TO_PATTERN_IMPLEMENTATION,
            ),
        ]
        .iter()
        .zip(&fixed.capabilities[7..])
        .any(|((kind, implementation), capability)| {
            capability.kind != *kind
                || capability.contract_revision != conduit_text::MORSE_COMPOSITION_CONTRACT_REVISION
                || capability.implementation != *implementation
                || capability.required_base != crate::machine::BaseKind::Memory
        })
        || [
            (
                conduit_time::TIME_EVERY_KIND,
                conduit_time::TIME_EVERY_CONTRACT_REVISION,
                crate::offer::TIME_EVERY_IMPLEMENTATION,
            ),
            (
                conduit_semantic_catalog::STATE_COUNT_KIND,
                conduit_semantic_catalog::STATE_COUNT_CONTRACT_REVISION,
                crate::offer::STATE_COUNT_IMPLEMENTATION,
            ),
            (
                conduit_semantic_catalog::COUNT_PRESENTATION_KIND,
                conduit_semantic_catalog::COUNT_PRESENTATION_CONTRACT_REVISION,
                crate::offer::COUNT_PRESENTATION_IMPLEMENTATION,
            ),
        ]
        .iter()
        .zip(&fixed.capabilities[12..])
        .any(|((kind, revision, implementation), capability)| {
            capability.kind != *kind
                || capability.contract_revision != *revision
                || capability.implementation != *implementation
        })
        || fixed.capabilities[2].required_base != crate::machine::BaseKind::Memory
        || fixed.capabilities[2].host_call.is_some()
        || fixed.capabilities[2].maximum_output_bytes != conduit_text::MAX_TEXT_BYTES
        || fixed.capabilities[2].output.is_none_or(|port| {
            port.name != "text"
                || port.value_kind != conduit_semantic_catalog::TEXT_PRESENTATION_VALUE_KIND
                || port.direction != crate::offer::PortDirection::Output
        })
        || fixed.capabilities[3].required_base != crate::machine::BaseKind::Memory
        || fixed.capabilities[3].host_call != Some(crate::functional_offers::TEXT_UPPER_HOST_CALL)
        || fixed.capabilities[3].maximum_input_bytes != conduit_text::MAX_TEXT_BYTES
        || fixed.capabilities[3].maximum_output_bytes != conduit_text::MAX_TEXT_BYTES
        || fixed.capabilities[4].required_base != crate::machine::BaseKind::Serial
        || fixed.capabilities[4].host_call != Some("conduit.host/present@1")
        || fixed.capabilities[4].maximum_input_bytes != crate::offer::SERIAL_MAXIMUM_BYTES
        || fixed.capabilities[4].input.is_none_or(|port| {
            port.name != "text"
                || port.value_kind != conduit_semantic_catalog::TEXT_PRESENTATION_VALUE_KIND
                || port.direction != crate::offer::PortDirection::Input
        })
        || fixed
            .capabilities
            .iter()
            .any(|capability| capability.artifact_build != build_id)
    {
        return Err(PreparationError::OfferMismatch);
    }
    // Planning receives only capability truth whose exact Base provider is
    // currently ready; co-residence in this host is not authority.
    for capability in [
        &fixed.capabilities[2],
        &fixed.capabilities[3],
        &fixed.capabilities[4],
        &fixed.capabilities[5],
        &fixed.capabilities[6],
        &fixed.capabilities[7],
        &fixed.capabilities[8],
        &fixed.capabilities[9],
        &fixed.capabilities[10],
        &fixed.capabilities[11],
        &fixed.capabilities[12],
        &fixed.capabilities[13],
        &fixed.capabilities[14],
    ] {
        fixed
            .capability_provider(capability)
            .map_err(|_| PreparationError::OfferMismatch)?;
    }
    let mut literal = crate::functional_offers::text_literal_offer();
    bind_native_capability(
        &mut literal,
        &fixed.capabilities[2],
        build_id,
        "text-literal",
    );
    let mut upper = crate::functional_offers::text_upper_offer();
    bind_native_capability(&mut upper, &fixed.capabilities[3], build_id, "text-upper");
    let mut presentation = crate::presentation_offers::presentation_offer_for(
        conduit_semantic_catalog::TEXT_PRESENTATION_KIND,
    )
    .expect("ConduitOS owns text presentation");
    bind_native_capability(
        &mut presentation,
        &fixed.capabilities[4],
        build_id,
        "presentation-text",
    );
    let mut morse = crate::functional_offers::text_morse_offer();
    bind_native_capability(&mut morse, &fixed.capabilities[5], build_id, "text-morse");
    let mut indicator = crate::functional_offers::indicator_presentation_offer();
    bind_native_capability(
        &mut indicator,
        &fixed.capabilities[6],
        build_id,
        "presentation-indicator",
    );
    let mut composition = crate::functional_offers::morse_composition_offers();
    for ((index, capability), name) in composition.iter_mut().enumerate().zip([
        "text-characters",
        "morse-lookup",
        "morse-intersperse",
        "morse-flatten",
        "morse-symbols-to-pattern",
    ]) {
        bind_native_capability(capability, &fixed.capabilities[7 + index], build_id, name);
    }
    let mut every = crate::functional_offers::time_every_offer();
    bind_native_capability(&mut every, &fixed.capabilities[12], build_id, "time-every");
    let mut count = crate::functional_offers::state_count_offer();
    bind_native_capability(&mut count, &fixed.capabilities[13], build_id, "state-count");
    let mut count_presentation = crate::presentation_offers::presentation_offer_for(
        conduit_semantic_catalog::COUNT_PRESENTATION_KIND,
    )
    .expect("ConduitOS owns count presentation");
    bind_native_capability(
        &mut count_presentation,
        &fixed.capabilities[14],
        build_id,
        "presentation-count",
    );
    let mut advertisement = HostAdvertisement {
        protocol_version: PROTOCOL_VERSION,
        host_id: HostId::from(hex_identity(&identities.host)),
        boot_id: BootId::from(hex_identity(&identities.boot)),
        offer_generation: OfferGeneration(fixed.generation),
        profile: HostProfileId::from(fixed.profile),
        bases: vec![],
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
        capabilities: vec![literal, upper, presentation, morse, indicator],
        planner_capabilities: Vec::new(),
    };
    advertisement.capabilities.append(&mut composition);
    advertisement
        .capabilities
        .extend([every, count, count_presentation]);
    crate::ordinary_base::append_serial(&mut advertisement, fixed)?;
    if let Some(keyboard) = fixed.keyboard {
        crate::keyboard_offer::append_to_advertisement(&mut advertisement, keyboard, build_id)
            .map_err(|_| PreparationError::OfferMismatch)?;
    }
    #[cfg(target_arch = "x86_64")]
    if let Some(pointer) = fixed.pointer {
        crate::pointer_offer::append_to_advertisement(&mut advertisement, pointer, build_id)
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
    plot: &conduit_plot::ExpandedCanonicalPlot,
    cord_bytes: u32,
) -> Result<(), PreparationError> {
    let literal = plot
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
        .ok_or(PreparationError::PlotRejected)?;
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
    #[allow(unused_mut)]
    let mut memory_bytes = 4096;
    #[cfg(conduitos_protected_execution)]
    if fixed.kind == conduit_text::TEXT_UPPER_KIND {
        memory_bytes +=
            crate::arch::TextDomain::RESERVED_BYTES + crate::text_protection::ROOT_METADATA_CEILING;
    }
    portable
        .resource_requirements
        .push(conduit_core::resource_requirement(
            "conduit.resource/runtime-memory@1",
            memory_bytes,
        ));
    portable.resource_requirements.sort();
}

#[cfg(test)]
mod tests;
