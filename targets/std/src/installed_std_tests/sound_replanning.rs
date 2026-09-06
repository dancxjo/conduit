use super::RecordingTimer;
use crate::hosted_audio::{
    AlsaPlaybackObservation, FakePlaybackBehavior, HostedPlaybackSelection, PlaybackLifecycle,
};
use crate::{StdHost, StdHostComposition, StdHostConfig};
use conduit_audio::NoteOccurrenceId;
use conduit_core::{
    bind_active_play, BaseImplementationId, BootId, HostId, OfferGeneration, SignId,
};
use conduit_planner::{
    replan_selected_realizations_with_characteristics, PlanningOptions, RealizationReplanOutcome,
    SelectedRealizationPlanning,
};
use std::collections::BTreeMap;

fn host(
    boot: &str,
    generation: u64,
    base_identity: &str,
    behavior: FakePlaybackBehavior,
) -> StdHost {
    let config = StdHostConfig {
        host_id: HostId::from("sound-replan-host"),
        boot_id: BootId::from(boot),
        offer_generation: OfferGeneration(generation),
    };
    let selection = HostedPlaybackSelection::deterministic_fake(
        AlsaPlaybackObservation {
            card_index: 0,
            card_id: "SAME_NAME".into(),
            card_name: "Same friendly device".into(),
            device: 0,
            device_name: "Same friendly endpoint".into(),
            base_identity: base_identity.into(),
        },
        config.boot_id.clone(),
        config.offer_generation,
        behavior,
    );
    StdHost::new_with_playback(config, StdHostComposition::reference(), selection)
        .expect("selection is scoped to the exact Host generation")
}

fn form() -> conduit_form::CheckedForm {
    conduit_form::parse(
        "form sound_replan {\n source: conduit-proof/pcm-specimen-source\n output: audio/play\n source.audio > output.audio\n}\n",
        &crate::installed_std::test_catalog(),
    )
    .expect("portable PCM Form is valid")
}

fn plan(host: &StdHost, form: &conduit_form::CheckedForm) -> conduit_core::Plan {
    let hosts = [host.advertisement().clone()];
    let selection = host.playback.as_ref().expect("playback selection exists");
    let advertisements = [selection.realization_advertisement(hosts[0].host_id.clone())];
    let observations = [selection.resource_observation(
        hosts[0].host_id.clone(),
        SignId::from(format!("sign/{}/playback-ready", hosts[0].boot_id.as_str())),
    )];
    let grant_id = format!("grant/{}/playback", hosts[0].boot_id.as_str());
    let grants = [host
        .playback_authority_grant(&grant_id)
        .expect("exact playback authority grant")];
    conduit_planner::plan_selected_realizations_with_characteristics_and_authority(
        form,
        SelectedRealizationPlanning {
            hosts: &hosts,
            bases: &[BaseImplementationId::from("conduit.base/local@1")],
            requirements: &BTreeMap::new(),
            advertisements: &advertisements,
            observations: &observations,
            policies: &BTreeMap::new(),
            connection_item_capacity: 1,
            connection_byte_capacity: conduit_semantic_catalog::AUDIO_PLAY_ALSA_PCM_BLOCK_BYTES,
            authority_grants: &grants,
        },
    )
    .expect("current endpoint plans")
}

#[test]
fn provider_loss_requires_a_fresh_plan_and_play_for_the_new_exact_endpoint() {
    let checked_form = form();
    let mut host_a = host(
        "sound-boot-a",
        7,
        "usb-path-a",
        FakePlaybackBehavior::ProviderLossAfterFirstBlock,
    );
    let plan_a = plan(&host_a, &checked_form);
    let immutable_plan_a = plan_a.clone();
    let fragment_a = &plan_a.fragments[0];
    let play_a = bind_active_play(&plan_a.plan_id, &fragment_a.host_id, &fragment_a.boot_id, 0);
    let mut musical_state = crate::sound_recovery::ActiveSoundState::<8>::new(
        plan_a.plan_id.clone(),
        play_a.active_play_id.clone(),
    );
    musical_state.note_on(NoteOccurrenceId(41)).unwrap();
    musical_state.note_on(NoteOccurrenceId(42)).unwrap();
    musical_state.set_sustain(true).unwrap();
    musical_state.set_pitch_bend(0, 9_000).unwrap();
    musical_state.set_controller(0, 1, 64).unwrap();

    let loss = host_a
        .run_fragment_to(
            plan_a.fragments[0].clone(),
            &mut Vec::with_capacity(2_048),
            &mut RecordingTimer { waits: Vec::new() },
        )
        .expect_err("active provider loss remains terminal and machine-readable");
    assert!(
        loss.contains("OperationFailed(Failure { code: HostOperationFailed, detail: 75 })"),
        "{loss}"
    );
    let interrupted = musical_state.provider_lost().unwrap();
    assert_eq!(interrupted.active_notes_interrupted, 2);
    assert!(interrupted.sustain_was_down);
    assert_eq!(interrupted.pitch_channels_reset, 1);
    assert_eq!(interrupted.controller_values_reset, 1);
    assert!(!interrupted.device_note_off_confirmed);
    assert!(!interrupted.drain_confirmed);

    let mut host_b = host(
        "sound-boot-b",
        8,
        "usb-path-b",
        FakePlaybackBehavior::Success,
    );
    let hosts_b = [host_b.advertisement().clone()];
    let selection_b = host_b.playback.as_ref().expect("replacement exists");
    let advertisements_b = [selection_b.realization_advertisement(hosts_b[0].host_id.clone())];
    let observations_b = [selection_b.resource_observation(
        hosts_b[0].host_id.clone(),
        SignId::from("sign/sound-boot-b/playback-ready"),
    )];
    let grants_b = [host_b
        .playback_authority_grant("grant/sound-boot-b/playback")
        .expect("replacement authority")];
    let connection_bases = BTreeMap::new();
    let line_candidates = BTreeMap::new();
    let outcome = replan_selected_realizations_with_characteristics(
        &plan_a,
        &checked_form,
        &hosts_b,
        &[BaseImplementationId::from("conduit.base/local@1")],
        &BTreeMap::new(),
        &advertisements_b,
        &observations_b,
        &BTreeMap::new(),
        PlanningOptions {
            connection_bases: &connection_bases,
            line_candidates: &line_candidates,
            connection_item_capacity: 1,
            connection_byte_capacity: conduit_semantic_catalog::AUDIO_PLAY_ALSA_PCM_BLOCK_BYTES,
            authority_grants: &grants_b,
            protected_resource_grants: &[],
            line_offers: &[],
        },
    )
    .expect("fresh observation permits ordinary replanning");
    let RealizationReplanOutcome::Replacement {
        previous_plan_id,
        plan: plan_b,
    } = outcome
    else {
        panic!("changed boot and exact endpoint must replace the Plan");
    };

    assert_eq!(plan_a, immutable_plan_a);
    assert_eq!(previous_plan_id, plan_a.plan_id);
    assert_ne!(plan_b.plan_id, plan_a.plan_id);
    assert_eq!(plan_b.source_document_id, plan_a.source_document_id);
    assert_eq!(plan_b.checked_form_id, plan_a.checked_form_id);
    assert_eq!(plan_b.expanded_form_id, plan_a.expanded_form_id);
    let playback_a = plan_a.fragments[0]
        .placements
        .iter()
        .find(|placement| placement.kind_id.as_str() == conduit_semantic_catalog::AUDIO_PLAY_KIND)
        .expect("Plan A playback placement");
    let playback_b = plan_b.fragments[0]
        .placements
        .iter()
        .find(|placement| placement.kind_id.as_str() == conduit_semantic_catalog::AUDIO_PLAY_KIND)
        .expect("Plan B playback placement");
    assert_ne!(playback_a.resources, playback_b.resources);
    assert_ne!(playback_a.authority, playback_b.authority);
    assert!(!playback_b.resources[0].pool_id.as_str().contains("default"));
    let fragment_b = &plan_b.fragments[0];
    let play_b = bind_active_play(&plan_b.plan_id, &fragment_b.host_id, &fragment_b.boot_id, 1);
    let mut replacement = crate::sound_recovery::ReplacementSoundState::<8>::start(
        &interrupted,
        plan_b.plan_id.clone(),
        play_b.active_play_id.clone(),
    )
    .unwrap();
    assert_eq!(replacement.state().active_note_count(), 0);
    assert!(!replacement.state().sustain_down());
    assert_eq!(replacement.state().changed_pitch_channel_count(), 0);
    assert_eq!(replacement.state().controller_value_count(), 0);
    replacement.expect_completion(17).unwrap();
    assert_eq!(
        replacement.accept_completion(&play_a.active_play_id, 17),
        Err(crate::sound_recovery::SoundRecoveryError::StaleCompletion)
    );
    replacement
        .accept_completion(&play_b.active_play_id, 17)
        .unwrap();

    let stale = host_b
        .run_fragment_to(
            plan_a.fragments[0].clone(),
            &mut Vec::with_capacity(2_048),
            &mut RecordingTimer { waits: Vec::new() },
        )
        .expect_err("old Plan cannot bind to a same-named replacement endpoint");
    assert!(stale.contains("current host boot and offer"), "{stale}");

    let report = host_b
        .run_fragment_to(
            plan_b.fragments[0].clone(),
            &mut Vec::with_capacity(2_048),
            &mut RecordingTimer { waits: Vec::new() },
        )
        .expect("fresh Plan starts a distinct successful Play");
    let playback = &report.kernel.expect("kernel report").playback[0];
    assert_eq!(playback.lifecycle, PlaybackLifecycle::StoppedClosed);
    assert_eq!(playback.metrics.blocks_committed, 96);
}

#[test]
fn replacement_refuses_old_or_absent_authority_and_loss_during_drain() {
    let checked_form = form();
    let host_a = host(
        "sound-authority-boot-a",
        9,
        "authority-path-a",
        FakePlaybackBehavior::Success,
    );
    let plan_a = plan(&host_a, &checked_form);
    let old_grant = host_a
        .playback_authority_grant("grant/device-a-only")
        .expect("device A authority");

    let host_b = host(
        "sound-authority-boot-b",
        10,
        "authority-path-b",
        FakePlaybackBehavior::Success,
    );
    let hosts_b = [host_b.advertisement().clone()];
    let selection_b = host_b.playback.as_ref().expect("replacement exists");
    let advertisements_b = [selection_b.realization_advertisement(hosts_b[0].host_id.clone())];
    let observations_b = [selection_b.resource_observation(
        hosts_b[0].host_id.clone(),
        SignId::from("sign/authority-device-b-ready"),
    )];
    let connection_bases = BTreeMap::new();
    let line_candidates = BTreeMap::new();
    for grants in [&[][..], std::slice::from_ref(&old_grant)] {
        let error = replan_selected_realizations_with_characteristics(
            &plan_a,
            &checked_form,
            &hosts_b,
            &[BaseImplementationId::from("conduit.base/local@1")],
            &BTreeMap::new(),
            &advertisements_b,
            &observations_b,
            &BTreeMap::new(),
            PlanningOptions {
                connection_bases: &connection_bases,
                line_candidates: &line_candidates,
                connection_item_capacity: 1,
                connection_byte_capacity: conduit_semantic_catalog::AUDIO_PLAY_ALSA_PCM_BLOCK_BYTES,
                authority_grants: grants,
                protected_resource_grants: &[],
                line_offers: &[],
            },
        )
        .expect_err("replacement must carry its own exact authority");
        assert!(
            matches!(
                error,
                conduit_planner::PlannerError::AuthorityGrantMissing(_)
            ),
            "{error:?}"
        );
    }

    let mut drain_loss = host(
        "sound-drain-loss-boot",
        11,
        "drain-loss-path",
        FakePlaybackBehavior::ProviderLossOnDrain,
    );
    let drain_plan = plan(&drain_loss, &checked_form);
    let error = drain_loss
        .run_fragment_to(
            drain_plan.fragments[0].clone(),
            &mut Vec::with_capacity(2_048),
            &mut RecordingTimer { waits: Vec::new() },
        )
        .expect_err("provider disappearance during drain remains device loss");
    assert!(
        error.contains("OperationFailed(Failure { code: HostOperationFailed, detail: 75 })"),
        "{error}"
    );
}
