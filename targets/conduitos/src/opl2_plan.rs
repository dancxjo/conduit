//! Ordinary planning for one exact boot-scoped OPL2 musical realization.

use alloc::{collections::BTreeMap, format, vec, vec::Vec};
use conduit_core::{
    ActivePlayIdentity, ArtifactId, BaseImplementationId, CapabilityId, CapabilityLimits,
    CapabilityOffer, CharacteristicId, ExecutionProfileId, GearId, HostAdvertisement,
    ImplementationId, KindContractRevision, Plan, PortDescriptor, PortDirection,
    RealizationAdvertisement, bind_active_play, kind_id, port_id,
};
use conduit_planner::{
    HardRealizationRequirements, SelectedRealizationPlanning,
    plan_selected_realizations_with_characteristics_and_authority,
};

use crate::{
    identity::BootIdentities,
    offer::HostOffer,
    opl2_offer::{
        OPL2_BASE_RESOURCE, OPL2_EVENT_RESOURCE, OPL2_EXECUTION_PROFILE, OPL2_IMPLEMENTATION,
        OPL2_STATE_BYTES, OPL2_VOICE_RESOURCE, OPL2_WRITE_RESOURCE, Opl2Offer,
    },
    ordinary_plan::{PreparationError, advertisement},
};

const NOTE_SOURCE_KIND: &str = "conduitos-fixture/note-source";
const NOTE_SOURCE_REVISION: &str = "conduitos.fixture/note-source@1";
const NOTE_SOURCE_PROFILE: &str = "conduitos/proof-note-source@1";
const NOTE_SOURCE_IMPLEMENTATION: &str = "conduitos.fixture/note-source@1";
const EMPTY_CONTROL_SOURCE_KIND: &str = "conduitos-fixture/empty-control-source";
const EMPTY_CONTROL_SOURCE_REVISION: &str = "conduitos.fixture/empty-control-source@1";
pub const NOTE_SOURCE_HOST_OPERATION: &str = "conduitos.fixture/note-sequence-step@1";
pub const OPL2_FORM_SOURCE: &str = "form conduitos-opl2-music {\n source: conduitos-fixture/note-source\n controls: conduitos-fixture/empty-control-source\n output: music/play\n source.notes > output.notes\n controls.controls > output.controls\n}\n";
pub const FIXTURE_EVENT_COUNT: u16 = 24;

pub struct PreparedOpl2Play {
    pub advertisement: HostAdvertisement,
    pub plan: Plan,
    pub active_play: ActivePlayIdentity,
}

pub fn prepare(
    identities: &BootIdentities,
    fixed: &HostOffer<'_>,
    opl2: Opl2Offer<'_>,
    build_id: &str,
) -> Result<PreparedOpl2Play, PreparationError> {
    opl2.validate(build_id)
        .map_err(|_| PreparationError::OfferMismatch)?;
    let expected_base = crate::identity::derive_base(&identities.boot, "conduitos/opl2/0");
    if opl2.realization.base_id != expected_base {
        return Err(PreparationError::OfferMismatch);
    }
    let mut host = advertisement(identities, fixed, build_id)?;
    crate::opl2_offer::append_to_advertisement(&mut host, opl2, build_id)
        .map_err(|_| PreparationError::OfferMismatch)?;
    host.capabilities.push(note_source_offer(build_id));
    host.capabilities.push(empty_control_source_offer(build_id));
    let form = checked_form()?;
    let realization = realization_advertisement(&host, opl2)?;
    let requirements = fixture_requirements();
    let observations = host
        .resources
        .iter()
        .enumerate()
        .map(|(index, pool)| conduit_core::ResourceObservation {
            host_id: host.host_id.clone(),
            boot_id: host.boot_id.clone(),
            offer_generation: host.offer_generation,
            pool_id: pool.pool_id.clone(),
            class_id: pool.class_id.clone(),
            health: conduit_core::ResourceHealth::Ready,
            unreserved_units: pool.capacity_units,
            utilized_units: 0,
            sign_id: conduit_core::SignId::from(format!("opl2-resource-{index}")),
        })
        .collect::<Vec<_>>();
    let hosts = [host.clone()];
    let plan = plan_selected_realizations_with_characteristics_and_authority(
        &form,
        SelectedRealizationPlanning {
            hosts: &hosts,
            bases: &[BaseImplementationId::from("conduit.base/local@1")],
            requirements: &requirements,
            advertisements: core::slice::from_ref(&realization),
            observations: &observations,
            policies: &BTreeMap::new(),
            connection_item_capacity: FIXTURE_EVENT_COUNT,
            connection_byte_capacity: u32::from(FIXTURE_EVENT_COUNT)
                * conduit_audio::NOTE_EVENT_ENCODED_LEN as u32,
            authority_grants: &[],
        },
    )
    .map_err(|_| PreparationError::PlanRejected)?;
    validate(&plan, &host, opl2, build_id)?;
    let fragment = &plan.fragments[0];
    let active_play = bind_active_play(
        &plan.plan_id,
        &fragment.host_id,
        &fragment.boot_id,
        u64::from(FIXTURE_EVENT_COUNT),
    );
    Ok(PreparedOpl2Play {
        advertisement: host,
        plan,
        active_play,
    })
}

pub fn validate(
    plan: &Plan,
    advertisement: &HostAdvertisement,
    opl2: Opl2Offer<'_>,
    build_id: &str,
) -> Result<(), PreparationError> {
    opl2.validate(build_id)
        .map_err(|_| PreparationError::OfferMismatch)?;
    if !conduit_core::verify_plan(plan) || plan.fragments.len() != 1 {
        return Err(PreparationError::PlanRejected);
    }
    let fragment = &plan.fragments[0];
    if fragment.host_id != advertisement.host_id
        || fragment.boot_id != advertisement.boot_id
        || fragment.offer_generation != advertisement.offer_generation
        || fragment.placements.len() != 3
        || fragment.connections.len() != 2
    {
        return Err(PreparationError::PlanRejected);
    }
    let sink = fragment
        .placements
        .iter()
        .find(|placement| placement.kind_id.as_str() == conduit_semantic_catalog::MUSIC_PLAY_KIND)
        .ok_or(PreparationError::PlanRejected)?;
    if sink.kind_contract_revision.as_str() != conduit_semantic_catalog::MUSIC_PLAY_REVISION
        || sink.execution_profile_id.as_str() != OPL2_EXECUTION_PROFILE
        || sink.implementation_id.as_str() != OPL2_IMPLEMENTATION
        || sink.artifact_id.as_str() != format!("conduitos-build/{build_id}")
        || sink.inputs != conduit_semantic_catalog::music_play_contract().inputs
        || !sink.outputs.is_empty()
        || sink.host_operations.len() != 1
        || sink.host_operations[0].contract_id.as_str() != crate::opl2_offer::OPL2_HOST_OPERATION
        || sink.realization_characteristics
            != conduit_semantic_catalog::sound_profile_characteristics(
                &crate::opl2_offer::compatibility_profile(),
            )
    {
        return Err(PreparationError::PlanRejected);
    }
    let realization = opl2.realization;
    let expected_resources = [
        (OPL2_BASE_RESOURCE, 1),
        (OPL2_VOICE_RESOURCE, u32::from(realization.channels)),
        (OPL2_EVENT_RESOURCE, u32::from(realization.event_slots)),
        (
            OPL2_WRITE_RESOURCE,
            u32::from(realization.register_write_slots),
        ),
        (
            conduit_core::RUNTIME_MEMORY_RESOURCE_CLASS,
            OPL2_STATE_BYTES,
        ),
    ];
    if sink.resources.len() != expected_resources.len()
        || expected_resources.iter().any(|(class, units)| {
            sink.resources
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
    if sink
        .resources
        .iter()
        .any(|binding| !pools.contains(&&binding.pool_id))
    {
        return Err(PreparationError::PlanRejected);
    }
    Ok(())
}

fn checked_form() -> Result<conduit_form::CheckedForm, PreparationError> {
    checked(OPL2_FORM_SOURCE)
}

fn checked(source: &str) -> Result<conduit_form::CheckedForm, PreparationError> {
    let mut catalog = conduit_form::ProfileCatalog::new();
    let mut startup = conduit_form::StartupCatalog::new();
    conduit_semantic_catalog::install_sound_catalogs(&mut startup, &mut catalog)
        .map_err(|_| PreparationError::FormRejected)?;
    catalog
        .insert(conduit_form::KindDefinition {
            kind_id: kind_id(NOTE_SOURCE_KIND),
            kind_contract_revision: KindContractRevision::from(NOTE_SOURCE_REVISION),
            inputs: Vec::new(),
            outputs: note_source_offer("catalog").outputs,
            configuration: Vec::new(),
        })
        .map_err(|_| PreparationError::FormRejected)?;
    catalog
        .insert(conduit_form::KindDefinition {
            kind_id: kind_id(EMPTY_CONTROL_SOURCE_KIND),
            kind_contract_revision: KindContractRevision::from(EMPTY_CONTROL_SOURCE_REVISION),
            inputs: Vec::new(),
            outputs: empty_control_source_offer("catalog").outputs,
            configuration: Vec::new(),
        })
        .map_err(|_| PreparationError::FormRejected)?;
    conduit_form::parse(source, &catalog).map_err(|_| PreparationError::FormRejected)
}

fn empty_control_source_offer(build_id: &str) -> CapabilityOffer {
    CapabilityOffer {
        startup_parameters: Vec::new(),
        shorthand: None,
        capability_id: CapabilityId::from("conduitos-fixture-empty-control-source@1"),
        kind_id: kind_id(EMPTY_CONTROL_SOURCE_KIND),
        kind_contract_revision: KindContractRevision::from(EMPTY_CONTROL_SOURCE_REVISION),
        inputs: Vec::new(),
        outputs: vec![PortDescriptor {
            port_id: port_id("controls"),
            value_kind: kind_id(conduit_audio::MUSIC_CONTROL_INFO_ID),
            direction: PortDirection::Output,
            temporal: conduit_core::PortTemporal::Value,
        }],
        implementation: conduit_core::ImplementationOffer {
            execution_profile_id: ExecutionProfileId::from(NOTE_SOURCE_PROFILE),
            implementation_id: ImplementationId::from("conduitos.fixture/empty-control-source@1"),
            artifact_id: ArtifactId::from(format!("conduitos-build/{build_id}")),
        },
        host_operations: Vec::new(),
        resource_requirements: Vec::new(),
        authority_requirements: Vec::new(),
        limits: CapabilityLimits {
            max_active_instances: 1,
            max_queue_items: FIXTURE_EVENT_COUNT,
            max_queue_bytes: u32::from(FIXTURE_EVENT_COUNT)
                * conduit_audio::NOTE_EVENT_ENCODED_LEN as u32,
        },
    }
}

fn note_source_offer(build_id: &str) -> CapabilityOffer {
    CapabilityOffer {
        startup_parameters: Vec::new(),
        shorthand: None,
        capability_id: CapabilityId::from("conduitos-fixture-note-source@1"),
        kind_id: kind_id(NOTE_SOURCE_KIND),
        kind_contract_revision: KindContractRevision::from(NOTE_SOURCE_REVISION),
        inputs: Vec::new(),
        outputs: vec![PortDescriptor {
            port_id: port_id("notes"),
            value_kind: kind_id(conduit_audio::MUSIC_NOTE_INFO_ID),
            direction: PortDirection::Output,
            temporal: conduit_core::PortTemporal::Value,
        }],
        implementation: conduit_core::ImplementationOffer {
            execution_profile_id: ExecutionProfileId::from(NOTE_SOURCE_PROFILE),
            implementation_id: ImplementationId::from(NOTE_SOURCE_IMPLEMENTATION),
            artifact_id: ArtifactId::from(format!("conduitos-build/{build_id}")),
        },
        host_operations: vec![conduit_core::HostOperationRequirement {
            contract_id: conduit_core::HostOperationContractId::from(NOTE_SOURCE_HOST_OPERATION),
            target_kind: None,
            maximum_in_flight: 1,
            maximum_input_bytes: 8,
            maximum_output_bytes: 0,
        }],
        resource_requirements: Vec::new(),
        authority_requirements: Vec::new(),
        limits: CapabilityLimits {
            max_active_instances: 1,
            max_queue_items: FIXTURE_EVENT_COUNT,
            max_queue_bytes: u32::from(FIXTURE_EVENT_COUNT)
                * conduit_audio::NOTE_EVENT_ENCODED_LEN as u32,
        },
    }
}

fn realization_advertisement(
    host: &HostAdvertisement,
    opl2: Opl2Offer<'_>,
) -> Result<RealizationAdvertisement, PreparationError> {
    let capability = host
        .capabilities
        .iter()
        .find(|capability| capability.capability_id.as_str() == crate::opl2_offer::OPL2_CAPABILITY)
        .ok_or(PreparationError::OfferMismatch)?;
    let profile = crate::opl2_offer::compatibility_profile();
    if profile.maximum_polyphony != opl2.realization.channels {
        return Err(PreparationError::OfferMismatch);
    }
    Ok(RealizationAdvertisement {
        host_id: host.host_id.clone(),
        boot_id: host.boot_id.clone(),
        offer_generation: host.offer_generation,
        capability_id: capability.capability_id.clone(),
        characteristics: conduit_semantic_catalog::sound_profile_characteristics(&profile),
    })
}

fn fixture_requirements() -> BTreeMap<GearId, HardRealizationRequirements> {
    let mut requirements = BTreeMap::new();
    let mut output = HardRealizationRequirements::default();
    output.required_characteristic_labels.insert(
        CharacteristicId::from(conduit_semantic_catalog::SOUND_SEAM_CHARACTERISTIC),
        "musical-events".into(),
    );
    output.minimum_characteristic_counts.insert(
        CharacteristicId::from(conduit_semantic_catalog::MUSIC_MAXIMUM_POLYPHONY_CHARACTERISTIC),
        conduit_core::CharacteristicQuantity {
            value: 3,
            unit: conduit_core::CharacteristicUnit::Items,
        },
    );
    output.required_characteristic_flags.insert(
        CharacteristicId::from(conduit_semantic_catalog::MUSIC_SUBTRACTIVE_FILTER_CHARACTERISTIC),
        false,
    );
    requirements.insert(GearId::from("conduitos-opl2-music/output"), output);
    requirements
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use crate::{offer::CpuFeatures, opl2_offer::Opl2Realization};

    pub(crate) fn fixture() -> (BootIdentities, HostOffer<'static>, Opl2Offer<'static>) {
        let identities = BootIdentities {
            host: [1; 32],
            boot: [2; 32],
        };
        let fixed = HostOffer::new(
            &identities,
            "build",
            CpuFeatures {
                sse2: true,
                rdrand: true,
                invariant_tsc: true,
            },
            1_048_576,
        );
        let opl2 = Opl2Offer {
            artifact_build: "build",
            realization: Opl2Realization {
                base_id: crate::identity::derive_base(&identities.boot, "conduitos/opl2/0"),
                clock_hz: crate::opl2_offer::OPL2_CLOCK_HZ,
                channels: crate::opl2_offer::OPL2_CHANNELS,
                maximum_error_parts_per_million: 2_500,
                event_slots: 32,
                register_write_slots: 512,
                patch_profile: crate::opl2_offer::OPL2_PATCH_PROFILE,
            },
        };
        (identities, fixed, opl2)
    }

    #[test]
    fn exact_opl_profile_is_sealed_into_an_ordinary_music_plan() {
        let (identities, fixed, opl2) = fixture();
        let prepared = prepare(&identities, &fixed, opl2, "build").unwrap();
        assert_eq!(prepared.plan.fragments[0].placements.len(), 3);
        assert_eq!(prepared.plan.fragments[0].connections.len(), 2);
        assert_eq!(
            validate(&prepared.plan, &prepared.advertisement, opl2, "build"),
            Ok(())
        );
    }

    #[test]
    fn absent_stale_wrong_base_and_insufficient_work_refuse_before_play() {
        let (identities, fixed, opl2) = fixture();
        let mut wrong = opl2;
        wrong.realization.base_id = [8; 32];
        assert_eq!(
            prepare(&identities, &fixed, wrong, "build").err(),
            Some(PreparationError::OfferMismatch)
        );
        let mut stale = identities;
        stale.boot = [9; 32];
        assert_eq!(
            prepare(&stale, &fixed, opl2, "build").err(),
            Some(PreparationError::OfferMismatch)
        );
        let mut pressured = opl2;
        pressured.realization.register_write_slots = 400;
        assert_eq!(
            prepare(&identities, &fixed, pressured, "build").err(),
            Some(PreparationError::OfferMismatch)
        );
    }

    #[test]
    fn pcm_and_subtractive_controls_are_not_compatible_opl_requirements() {
        let offered = crate::opl2_offer::compatibility_profile();
        let mut subtractive = offered.clone();
        subtractive.supports_subtractive_filter = true;
        assert_eq!(
            conduit_semantic_catalog::compatibility(&subtractive, &offered),
            Err(conduit_semantic_catalog::IncompatibilityReason::SubtractiveFilterUnsupported)
        );
        let mut pcm = offered.clone();
        pcm.seam = conduit_semantic_catalog::SoundSeam::PcmPlayback;
        assert_eq!(
            conduit_semantic_catalog::compatibility(&pcm, &offered),
            Err(conduit_semantic_catalog::IncompatibilityReason::WrongSemanticSeam)
        );
    }
}
