use std::collections::BTreeMap;

use conduit_composite::CompositeDefinition;
use conduit_core::{
    kind_id, port_id, ArtifactId, BootId, CapabilityId, CapabilityLimits, CapabilityOffer,
    CharacteristicId, ConnectionBase, ExecutionProfileId, FailureReason, GearId, HostAdvertisement,
    HostId, HostProfileId, ImplementationId, ImplementationOffer, KindContractRevision,
    OfferGeneration, PortDescriptor, PortDirection, PortTemporal, RealizationAdvertisement,
    PROTOCOL_VERSION,
};
use conduit_form::{parse, KindDefinition, ProfileCatalog};
use conduit_planner::{
    default_placements, plan, plan_selected_realizations_with_characteristics,
    HardRealizationRequirements, PlacementChoice, PlacementChoices,
};
use conduit_std_catalog::{
    audio_play_contract, music_synth_contract, sound_profile_characteristics,
    SoundCompatibilityProfile, SoundSeam, AUDIO_PLAY_KIND, AUDIO_PLAY_REVISION,
    MUSIC_MAXIMUM_POLYPHONY_CHARACTERISTIC, MUSIC_PLAY_KIND, MUSIC_SYNTH_KIND,
    MUSIC_SYNTH_REVISION, SOUND_SEAM_CHARACTERISTIC,
};

fn offer(
    contract: conduit_std_catalog::StandardKindContract,
    revision: &str,
    id: &str,
) -> CapabilityOffer {
    CapabilityOffer {
        startup_parameters: Vec::new(),
        shorthand: None,
        capability_id: CapabilityId::from(id),
        kind_id: contract.kind_id,
        kind_contract_revision: KindContractRevision::from(revision),
        implementation: ImplementationOffer {
            execution_profile_id: ExecutionProfileId::from("test/sound-reference@1"),
            implementation_id: ImplementationId::from(format!("test/{id}-implementation@1")),
            artifact_id: ArtifactId::from(format!("test/{id}-artifact@1")),
        },
        inputs: contract.inputs,
        outputs: contract.outputs,
        host_operations: Vec::new(),
        resource_requirements: Vec::new(),
        authority_requirements: Vec::new(),
        limits: CapabilityLimits {
            max_active_instances: 1,
            max_queue_items: 8,
            max_queue_bytes: 524_288,
        },
    }
}

fn host(id: &str, capabilities: Vec<CapabilityOffer>) -> HostAdvertisement {
    HostAdvertisement {
        protocol_version: PROTOCOL_VERSION,
        host_id: HostId::from(id),
        boot_id: BootId::from(format!("{id}-boot")),
        offer_generation: OfferGeneration(1),
        profile: HostProfileId::from("test/sound-host@1"),
        resources: Vec::new(),
        capabilities,
        planner_capabilities: Vec::new(),
    }
}

fn internal_catalog() -> ProfileCatalog {
    let mut catalog = ProfileCatalog::new();
    for (contract, revision) in [
        (music_synth_contract(), MUSIC_SYNTH_REVISION),
        (audio_play_contract(), AUDIO_PLAY_REVISION),
    ] {
        catalog
            .insert(KindDefinition {
                kind_id: contract.kind_id,
                kind_contract_revision: KindContractRevision::from(revision),
                inputs: contract.inputs,
                outputs: contract.outputs,
                configuration: Vec::new(),
            })
            .unwrap();
    }
    catalog
}

fn internal_form() -> conduit_form::CheckedForm {
    parse(
        "form 0\nrealize-music {\n synth: music/synth\n playback: audio/play\n synth.audio -> playback.audio\n export play: music/play {\n  input notes: music/note-event@1 = synth.notes terminal independent\n  input controls: music/control-event@1 = synth.controls terminal independent\n }\n}\n",
        &internal_catalog(),
    )
    .expect("standard music realization Form checks")
}

fn internal_plan(form: &conduit_form::CheckedForm) -> conduit_core::Plan {
    let internal_host = host(
        "internal-sound-host",
        vec![
            offer(music_synth_contract(), MUSIC_SYNTH_REVISION, "synth"),
            offer(audio_play_contract(), AUDIO_PLAY_REVISION, "audio"),
        ],
    );
    let hosts = [internal_host];
    let placements = default_placements(form, &hosts).unwrap();
    plan(form, &hosts, &placements, &[ConnectionBase::Local]).unwrap()
}

fn source_definition(kind: &str, value_kind: &str) -> KindDefinition {
    KindDefinition {
        kind_id: kind_id(kind),
        kind_contract_revision: KindContractRevision::from(format!("{kind}@1")),
        inputs: Vec::new(),
        outputs: vec![PortDescriptor {
            port_id: port_id("out"),
            value_kind: kind_id(value_kind),
            direction: PortDirection::Output,
            temporal: PortTemporal::Value,
        }],
        configuration: Vec::new(),
    }
}

#[test]
fn unchanged_authored_music_plans_directly_or_through_recursive_standard_form() {
    let realization = internal_form();
    let internal_plan = internal_plan(&realization);
    let composite = CompositeDefinition::from_authored_export(
        HostId::from("recursive-music-host"),
        BootId::from("recursive-music-boot"),
        OfferGeneration(1),
        HostProfileId::from("test/recursive-music@1"),
        ImplementationId::from("standard/music-synth-audio-play@1"),
        ArtifactId::from("conduit.std/music-synth-audio-play@1"),
        &realization,
        &CapabilityId::from("play"),
        internal_plan.clone(),
        FailureReason::CompositeCapabilityFailed,
    )
    .expect("ordinary composite derives a music/play offer");

    let mut parent_catalog = ProfileCatalog::new();
    parent_catalog
        .insert(source_definition("test/note-source", "music/note-event@1"))
        .unwrap();
    parent_catalog
        .insert(source_definition(
            "test/control-source",
            "music/control-event@1",
        ))
        .unwrap();
    parent_catalog
        .insert_export(&realization, &CapabilityId::from("play"))
        .unwrap();
    let source = "form 0\nperformance {\n notes: test/note-source\n controls: test/control-source\n output: music/play\n notes.out -> output.notes\n controls.out -> output.controls\n}\n";
    let authored = parse(source, &parent_catalog).expect("portable authored music checks");

    let note_offer = offer_for_source(&parent_catalog, "test/note-source", "notes");
    let control_offer = offer_for_source(&parent_catalog, "test/control-source", "controls");
    let mut direct_capability = composite.external_capability.clone();
    direct_capability.capability_id = CapabilityId::from("direct-music");
    direct_capability.implementation = ImplementationOffer {
        execution_profile_id: ExecutionProfileId::from("test/direct-musical-events@1"),
        implementation_id: ImplementationId::from("test/direct-music-device@1"),
        artifact_id: ArtifactId::from("test/direct-music-device-artifact@1"),
    };
    let direct = host(
        "direct-music-host",
        vec![note_offer.clone(), control_offer.clone(), direct_capability],
    );
    let direct_profile = SoundCompatibilityProfile {
        profile_id: "test/direct-polyphonic@1".into(),
        seam: SoundSeam::MusicalEvents,
        minimum_pitch_millihertz: 8_000,
        maximum_pitch_millihertz: 40_000_000,
        maximum_polyphony: 8,
        maximum_events_per_second: 2_000,
        preserves_velocity: true,
        preserves_sustain: true,
        preserves_pitch_bend: true,
        maximum_pitch_bend_range_microcents: 200_000_000,
        preserves_modulation: true,
        accepts_microtonal_pitch: true,
        supports_subtractive_filter: false,
        pcm: None,
    };
    let direct_advertisement = RealizationAdvertisement {
        host_id: direct.host_id.clone(),
        boot_id: direct.boot_id.clone(),
        offer_generation: direct.offer_generation,
        capability_id: CapabilityId::from("direct-music"),
        characteristics: sound_profile_characteristics(&direct_profile),
    };
    let mut requirements = BTreeMap::new();
    let mut output_requirement = HardRealizationRequirements::default();
    output_requirement.required_characteristic_labels.insert(
        CharacteristicId::from(SOUND_SEAM_CHARACTERISTIC),
        "musical-events".into(),
    );
    output_requirement.minimum_characteristic_counts.insert(
        CharacteristicId::from(MUSIC_MAXIMUM_POLYPHONY_CHARACTERISTIC),
        conduit_core::CharacteristicQuantity {
            value: 8,
            unit: conduit_core::CharacteristicUnit::Items,
        },
    );
    requirements.insert(GearId::from("output"), output_requirement);
    let characteristic_plan = plan_selected_realizations_with_characteristics(
        &authored,
        core::slice::from_ref(&direct),
        &[ConnectionBase::Local],
        &requirements,
        core::slice::from_ref(&direct_advertisement),
        &[],
        &BTreeMap::new(),
    )
    .expect("sound profile is admitted through canonical planner characteristics");
    let planned_output = characteristic_plan
        .fragments
        .iter()
        .flat_map(|fragment| &fragment.placements)
        .find(|placement| placement.kind_id.as_str() == MUSIC_PLAY_KIND)
        .unwrap();
    assert_eq!(
        planned_output.realization_characteristics,
        direct_advertisement.characteristics
    );
    let recursive = host(
        "recursive-music-host",
        vec![
            note_offer,
            control_offer,
            composite.external_capability.clone(),
        ],
    );
    let hosts = [direct, recursive];
    let base = default_placements(&authored, &hosts).unwrap();
    let direct_plan = plan_with_host(
        &authored,
        &hosts,
        base.clone(),
        "direct-music-host",
        "direct-music",
    );
    let recursive_plan = plan_with_host(&authored, &hosts, base, "recursive-music-host", "play");

    assert_eq!(
        direct_plan.source_document_id,
        recursive_plan.source_document_id
    );
    assert_eq!(direct_plan.checked_form_id, recursive_plan.checked_form_id);
    assert_eq!(
        direct_plan.expanded_form_id,
        recursive_plan.expanded_form_id
    );
    assert_ne!(direct_plan.plan_id, recursive_plan.plan_id);
    assert_eq!(
        internal_plan.source_document_id,
        realization.source_document_id
    );
    assert!(internal_plan
        .fragments
        .iter()
        .flat_map(|fragment| &fragment.placements)
        .any(|placement| placement.kind_id.as_str() == MUSIC_SYNTH_KIND));
    assert!(internal_plan
        .fragments
        .iter()
        .flat_map(|fragment| &fragment.placements)
        .any(|placement| placement.kind_id.as_str() == AUDIO_PLAY_KIND));
}

fn offer_for_source(catalog: &ProfileCatalog, kind: &str, id: &str) -> CapabilityOffer {
    let definition = catalog.get(&kind_id(kind)).unwrap();
    CapabilityOffer {
        startup_parameters: Vec::new(),
        shorthand: None,
        capability_id: CapabilityId::from(id),
        kind_id: definition.kind_id.clone(),
        kind_contract_revision: definition.kind_contract_revision.clone(),
        implementation: ImplementationOffer {
            execution_profile_id: ExecutionProfileId::from("test/source@1"),
            implementation_id: ImplementationId::from(format!("test/{id}@1")),
            artifact_id: ArtifactId::from(format!("test/{id}-artifact@1")),
        },
        inputs: definition.inputs.clone(),
        outputs: definition.outputs.clone(),
        host_operations: Vec::new(),
        resource_requirements: Vec::new(),
        authority_requirements: Vec::new(),
        limits: CapabilityLimits {
            max_active_instances: 1,
            max_queue_items: 8,
            max_queue_bytes: 16_384,
        },
    }
}

fn plan_with_host(
    form: &conduit_form::CheckedForm,
    hosts: &[HostAdvertisement],
    mut choices: PlacementChoices,
    host: &str,
    capability: &str,
) -> conduit_core::Plan {
    choices.by_gear.insert(
        GearId::from("notes"),
        PlacementChoice {
            host_id: HostId::from(host),
            capability_id: CapabilityId::from("notes"),
        },
    );
    choices.by_gear.insert(
        GearId::from("controls"),
        PlacementChoice {
            host_id: HostId::from(host),
            capability_id: CapabilityId::from("controls"),
        },
    );
    choices.by_gear.insert(
        GearId::from("output"),
        PlacementChoice {
            host_id: HostId::from(host),
            capability_id: CapabilityId::from(capability),
        },
    );
    plan(form, hosts, &choices, &[ConnectionBase::Local]).unwrap()
}
