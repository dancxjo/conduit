//! Ordinary planning for one exact boot-scoped PC-speaker tone sink.

use alloc::{collections::BTreeMap, vec::Vec};
use conduit_core::{
    ActivePlayIdentity, ArtifactId, BaseImplementationId, CapabilityId, CapabilityLimits,
    CapabilityOffer, ExecutionProfileId, HostAdvertisement, ImplementationId, KindContractRevision,
    Plan, PortDescriptor, PortDirection, bind_active_play, kind_id, port_id,
};
use conduit_planner::{
    PlanningOptions, default_expanded_placements, plan_expanded_canonical_with_options,
};

use crate::{
    identity::BootIdentities,
    offer::HostOffer,
    ordinary_plan::{PreparationError, advertisement},
    pc_speaker_offer::{
        PC_SPEAKER_BASE_RESOURCE, PC_SPEAKER_EVENT_RESOURCE, PC_SPEAKER_EXECUTION_PROFILE,
        PC_SPEAKER_IMPLEMENTATION, PC_SPEAKER_OPERATION_RESOURCE, PC_SPEAKER_STATE_BYTES,
    },
};

const TONE_SOURCE_KIND: &str = "conduitos-fixture/tone-source";
const TONE_SOURCE_REVISION: &str = "conduitos.fixture/tone-source@1";
const TONE_SOURCE_PROFILE: &str = "conduitos/proof-tone-source@1";
const TONE_SOURCE_IMPLEMENTATION: &str = "conduitos.fixture/tone-source@1";
pub const TONE_SOURCE_HOST_OPERATION: &str = "conduitos.fixture/tone-sequence-step@1";
pub const PC_SPEAKER_FORM_SOURCE: &str = "form conduitos-tone {\n    source: conduitos-fixture/tone-source\n    speaker: sound/tone-play\n    source > speaker.tone\n}\n";

pub struct PreparedPcSpeakerPlay {
    pub advertisement: HostAdvertisement,
    pub plan: Plan,
    pub active_play: ActivePlayIdentity,
}

pub fn prepare(
    identities: &BootIdentities,
    offer: &HostOffer<'_>,
    build_id: &str,
) -> Result<PreparedPcSpeakerPlay, PreparationError> {
    if offer.pc_speaker.is_none() {
        return Err(PreparationError::PlacementRejected);
    }
    let mut advertisement = advertisement(identities, offer, build_id)?;
    advertisement.capabilities.push(tone_source_offer(build_id));
    let form = checked_expanded_form()?;
    let hosts = [advertisement.clone()];
    let placements = default_expanded_placements(&form, &hosts)
        .map_err(|_| PreparationError::PlacementRejected)?;
    let plan = plan_expanded_canonical_with_options(
        &form,
        &hosts,
        &placements,
        &[BaseImplementationId::from("conduit.base/local@1")],
        PlanningOptions {
            connection_bases: &BTreeMap::new(),
            line_candidates: &BTreeMap::new(),
            connection_item_capacity: 1,
            connection_byte_capacity: conduit_audio::TONE_INTENT_ENCODED_LEN as u32,
            authority_grants: &[],
            protected_resource_grants: &[],
            line_offers: &[],
        },
    )
    .map_err(|_| PreparationError::PlanRejected)?;
    validate(&plan, &advertisement, offer, build_id)?;
    let fragment = &plan.fragments[0];
    let active_play = bind_active_play(&plan.plan_id, &fragment.host_id, &fragment.boot_id, 4);
    Ok(PreparedPcSpeakerPlay {
        advertisement,
        plan,
        active_play,
    })
}

pub fn validate(
    plan: &Plan,
    advertisement: &HostAdvertisement,
    offer: &HostOffer<'_>,
    build_id: &str,
) -> Result<(), PreparationError> {
    let pc_speaker = offer.pc_speaker.ok_or(PreparationError::OfferMismatch)?;
    pc_speaker
        .validate(build_id)
        .map_err(|_| PreparationError::OfferMismatch)?;
    if !conduit_core::verify_plan(plan)
        || plan.fragments.len() != 1
        || advertisement.host_id.as_str() != crate::identity::hex(&offer.host_id)
        || advertisement.boot_id.as_str() != crate::identity::hex(&offer.boot_id)
        || advertisement.offer_generation.0 != offer.generation
    {
        return Err(PreparationError::PlanRejected);
    }
    let fragment = &plan.fragments[0];
    if fragment.host_id != advertisement.host_id
        || fragment.boot_id != advertisement.boot_id
        || fragment.offer_generation != advertisement.offer_generation
        || fragment.placements.len() != 2
        || fragment.connections.len() != 1
    {
        return Err(PreparationError::PlanRejected);
    }
    let placement = fragment
        .placements
        .iter()
        .find(|placement| {
            placement.kind_id.as_str() == conduit_semantic_catalog::SOUND_TONE_PLAY_KIND
        })
        .ok_or(PreparationError::PlanRejected)?;
    let expected_input = conduit_semantic_catalog::sound_tone_play_contract().inputs;
    if placement.kind_id.as_str() != conduit_semantic_catalog::SOUND_TONE_PLAY_KIND
        || placement.kind_contract_revision.as_str()
            != conduit_semantic_catalog::SOUND_TONE_PLAY_REVISION
        || placement.execution_profile_id.as_str() != PC_SPEAKER_EXECUTION_PROFILE
        || placement.implementation_id.as_str() != PC_SPEAKER_IMPLEMENTATION
        || placement.artifact_id.as_str() != alloc::format!("conduitos-build/{build_id}")
        || placement.inputs != expected_input
        || !placement.outputs.is_empty()
        || placement.host_operations.len() != 1
        || placement.host_operations[0].contract_id.as_str()
            != crate::pc_speaker_offer::PC_SPEAKER_HOST_OPERATION
    {
        return Err(PreparationError::PlanRejected);
    }
    let expected_resources = [
        (PC_SPEAKER_BASE_RESOURCE, 1),
        (
            PC_SPEAKER_EVENT_RESOURCE,
            u32::from(pc_speaker.realization.event_slots),
        ),
        (PC_SPEAKER_OPERATION_RESOURCE, 1),
        (
            conduit_core::RUNTIME_MEMORY_RESOURCE_CLASS,
            PC_SPEAKER_STATE_BYTES,
        ),
    ];
    if placement.resources.len() != expected_resources.len()
        || expected_resources.iter().any(|(class, units)| {
            placement
                .resources
                .iter()
                .filter(|binding| binding.class_id.as_str() == *class && binding.units == *units)
                .count()
                != 1
        })
    {
        return Err(PreparationError::PlanRejected);
    }
    let pools = advertisement
        .resources
        .iter()
        .map(|resource| &resource.pool_id)
        .collect::<Vec<_>>();
    if placement
        .resources
        .iter()
        .any(|binding| !pools.contains(&&binding.pool_id))
    {
        return Err(PreparationError::PlanRejected);
    }
    let base = crate::identity::hex(&pc_speaker.realization.base_id);
    for suffix in ["base", "events", "operation"] {
        let expected = alloc::format!("conduitos-pc-speaker-{suffix}-{base}");
        if !advertisement
            .resources
            .iter()
            .any(|resource| resource.pool_id.as_str() == expected)
        {
            return Err(PreparationError::OfferMismatch);
        }
    }
    Ok(())
}

fn checked_expanded_form() -> Result<conduit_form::ExpandedCanonicalForm, PreparationError> {
    checked_expanded(PC_SPEAKER_FORM_SOURCE, "conduitos-tone")
}

fn checked_expanded(
    source: &str,
    form_name: &str,
) -> Result<conduit_form::ExpandedCanonicalForm, PreparationError> {
    let syntax = conduit_form::parse_syntax_document(source);
    let mut startup = conduit_form::StartupCatalog::new();
    let mut profile = conduit_form::ProfileCatalog::new();
    conduit_semantic_catalog::install_sound_catalogs(&mut startup, &mut profile)
        .map_err(|_| PreparationError::FormRejected)?;
    startup
        .insert(conduit_form::KindSignature {
            kind: TONE_SOURCE_KIND.into(),
            startup_parameters: Vec::new(),
        })
        .map_err(|_| PreparationError::FormRejected)?;
    profile
        .insert(conduit_form::KindDefinition {
            kind_id: kind_id(TONE_SOURCE_KIND),
            kind_contract_revision: KindContractRevision::from(TONE_SOURCE_REVISION),
            inputs: Vec::new(),
            outputs: tone_source_offer("catalog").outputs,
            configuration: Vec::new(),
        })
        .map_err(|_| PreparationError::FormRejected)?;
    let checked = conduit_form::check_syntax_document(&syntax, &startup)
        .map_err(|_| PreparationError::FormRejected)?;
    conduit_form::expand_canonical_form(&checked, form_name, &profile)
        .map_err(|_| PreparationError::FormRejected)
}

fn tone_source_offer(build_id: &str) -> CapabilityOffer {
    CapabilityOffer {
        startup_parameters: Vec::new(),
        shorthand: None,
        capability_id: CapabilityId::from("conduitos-fixture-tone-source@1"),
        kind_id: kind_id(TONE_SOURCE_KIND),
        kind_contract_revision: KindContractRevision::from(TONE_SOURCE_REVISION),
        inputs: Vec::new(),
        outputs: alloc::vec![PortDescriptor {
            port_id: port_id("tone"),
            value_kind: kind_id(conduit_audio::SOUND_TONE_INFO_ID),
            direction: PortDirection::Output,
            temporal: conduit_core::PortTemporal::Value,
        }],
        implementation: conduit_core::ImplementationOffer {
            execution_profile_id: ExecutionProfileId::from(TONE_SOURCE_PROFILE),
            implementation_id: ImplementationId::from(TONE_SOURCE_IMPLEMENTATION),
            artifact_id: ArtifactId::from(alloc::format!("conduitos-build/{build_id}")),
        },
        host_operations: alloc::vec![conduit_core::HostOperationRequirement {
            contract_id: conduit_core::HostOperationContractId::from(TONE_SOURCE_HOST_OPERATION),
            target_kind: None,
            maximum_in_flight: 1,
            maximum_input_bytes: 8,
            maximum_output_bytes: 0,
        }],
        resource_requirements: Vec::new(),
        authority_requirements: Vec::new(),
        limits: CapabilityLimits {
            max_active_instances: 1,
            max_queue_items: 4,
            max_queue_bytes: 4 * conduit_audio::TONE_INTENT_ENCODED_LEN as u32,
        },
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use crate::{
        offer::{CpuFeatures, HostOffer},
        pc_speaker_offer::PcSpeakerRealization,
    };

    pub(crate) fn fixture() -> (BootIdentities, HostOffer<'static>) {
        let identities = BootIdentities {
            host: [1; 32],
            boot: [2; 32],
        };
        let base_id = crate::identity::derive_base(&identities.boot, "conduitos/pc-speaker/0");
        let offer = HostOffer::new(
            &identities,
            "build",
            CpuFeatures {
                sse2: true,
                rdrand: true,
                invariant_tsc: true,
            },
            1_048_576,
        )
        .with_pc_speaker(
            PcSpeakerRealization {
                base_id,
                pit_input_hz: 1_193_182,
                minimum_divisor: 19,
                maximum_divisor: u16::MAX,
                maximum_error_parts_per_million: 2_500,
                event_slots: 8,
                operation_slots: 1,
            },
            "build",
        )
        .unwrap();
        (identities, offer)
    }

    #[test]
    fn ready_base_produces_one_exact_ordinary_tone_placement() {
        let (identities, offer) = fixture();
        let prepared = prepare(&identities, &offer, "build").unwrap();
        assert_eq!(prepared.plan.fragments[0].placements.len(), 2);
        let sink = prepared.plan.fragments[0]
            .placements
            .iter()
            .find(|placement| {
                placement.kind_id.as_str() == conduit_semantic_catalog::SOUND_TONE_PLAY_KIND
            })
            .unwrap();
        assert_eq!(sink.resources.len(), 4);
    }

    #[test]
    fn absent_stale_conflicting_and_richer_semantics_refuse() {
        let (identities, offer) = fixture();
        let absent = HostOffer::new(
            &identities,
            "build",
            offer.cpu_features,
            offer.runtime_arena_bytes,
        );
        assert_eq!(
            prepare(&identities, &absent, "build").err(),
            Some(PreparationError::PlacementRejected)
        );
        let prepared = prepare(&identities, &offer, "build").unwrap();
        let mut stale = offer;
        stale.boot_id = [9; 32];
        assert_eq!(
            validate(&prepared.plan, &prepared.advertisement, &stale, "build"),
            Err(PreparationError::PlanRejected)
        );
        let (_, mut wrong_base) = fixture();
        wrong_base.pc_speaker.as_mut().unwrap().realization.base_id = [8; 32];
        assert_eq!(
            validate(
                &prepared.plan,
                &prepared.advertisement,
                &wrong_base,
                "build"
            ),
            Err(PreparationError::OfferMismatch)
        );

        let syntax = conduit_form::parse_syntax_document(
            "form invalid (\n > audio: audio/pcm-frame@1\n) {\n speaker: sound/tone-play\n audio > speaker.tone\n}\n",
        );
        let mut startup = conduit_form::StartupCatalog::new();
        let mut profile = conduit_form::ProfileCatalog::new();
        conduit_semantic_catalog::install_sound_catalogs(&mut startup, &mut profile).unwrap();
        let checked = conduit_form::check_syntax_document(&syntax, &startup).unwrap();
        assert!(conduit_form::expand_canonical_form(&checked, "invalid", &profile).is_err());
        assert!(
            prepared
                .advertisement
                .capabilities
                .iter()
                .all(|capability| capability.kind_id.as_str()
                    != conduit_semantic_catalog::MUSIC_PLAY_KIND
                    && capability.kind_id.as_str() != conduit_semantic_catalog::AUDIO_PLAY_KIND)
        );
    }

    #[test]
    fn exclusive_base_and_operation_resources_refuse_a_second_tone_sink() {
        let (identities, offer) = fixture();
        let form = checked_expanded(
            "form conflict {\n left-source: conduitos-fixture/tone-source\n right-source: conduitos-fixture/tone-source\n left: sound/tone-play\n right: sound/tone-play\n left-source > left.tone\n right-source > right.tone\n}\n",
            "conflict",
        )
        .unwrap();
        let mut host = advertisement(&identities, &offer, "build").unwrap();
        let mut source = tone_source_offer("build");
        source.limits.max_active_instances = 2;
        host.capabilities.push(source);
        assert!(default_expanded_placements(&form, &[host]).is_err());
    }

    #[test]
    fn event_capacity_exhaustion_refuses_before_play() {
        let (identities, offer) = fixture();
        let form = checked_expanded_form().unwrap();
        let mut host = advertisement(&identities, &offer, "build").unwrap();
        host.capabilities.push(tone_source_offer("build"));
        let event_pool = host
            .resources
            .iter_mut()
            .find(|resource| resource.class_id.as_str() == PC_SPEAKER_EVENT_RESOURCE)
            .unwrap();
        event_pool.capacity_units = 7;
        let hosts = [host];
        let placements = default_expanded_placements(&form, &hosts).unwrap();
        assert!(
            plan_expanded_canonical_with_options(
                &form,
                &hosts,
                &placements,
                &[BaseImplementationId::from("conduit.base/local@1")],
                PlanningOptions {
                    connection_bases: &BTreeMap::new(),
                    line_candidates: &BTreeMap::new(),
                    connection_item_capacity: 1,
                    connection_byte_capacity: conduit_audio::TONE_INTENT_ENCODED_LEN as u32,
                    authority_grants: &[],
                    protected_resource_grants: &[],
                    line_offers: &[],
                },
            )
            .is_err()
        );
    }
}
