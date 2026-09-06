use super::RecordingTimer;
use crate::hosted_audio::{
    AlsaPlaybackObservation, FakePlaybackBehavior, HostedPlaybackSelection, PlaybackLifecycle,
};
use crate::{RunControl, RunControlRequestId, StdHost, StdHostComposition, StdHostConfig};
use conduit_core::{
    BaseImplementationId, BootId, HostId, ObservationKind, OfferGeneration, TerminalDisposition,
};
use std::collections::BTreeMap;

fn host(behavior: FakePlaybackBehavior) -> StdHost {
    let config = StdHostConfig {
        host_id: HostId::from("audio-fixture-host"),
        boot_id: BootId::from("audio-fixture-boot"),
        offer_generation: OfferGeneration(7),
    };
    let selection = HostedPlaybackSelection::deterministic_fake(
        AlsaPlaybackObservation {
            card_index: 0,
            card_id: "FIXTURE".into(),
            card_name: "Deterministic fixture".into(),
            device: 0,
            device_name: "Finite PCM sink".into(),
            base_identity: "fixture-base".into(),
        },
        config.boot_id.clone(),
        config.offer_generation,
        behavior,
    );
    StdHost::new_with_playback(config, StdHostComposition::reference(), selection)
        .expect("fixture selection matches exact Host identity")
}

fn form() -> conduit_form::CheckedForm {
    conduit_form::parse(
        "form audio_fixture {\n source: conduit-proof/pcm-specimen-source\n output: audio/play\n source.audio > output.audio\n}\n",
        &crate::installed_std::test_catalog(),
    )
    .expect("audio fixture Form is valid")
}

fn fragment(host: &StdHost, with_authority: bool) -> Result<conduit_core::PlanFragment, String> {
    let form = form();
    let advertisements = [host.advertisement().clone()];
    let grants = if with_authority {
        vec![host.playback_authority_grant("grant/test-audio-play")?]
    } else {
        Vec::new()
    };
    let realization = host
        .playback
        .as_ref()
        .expect("fixture host has playback")
        .realization_advertisement(host.advertisement().host_id.clone());
    let observation = host
        .playback
        .as_ref()
        .expect("fixture host has playback")
        .resource_observation(
            host.advertisement().host_id.clone(),
            conduit_core::SignId::from("sign/test-playback-ready"),
        );
    let plan = conduit_planner::plan_selected_realizations_with_characteristics_and_authority(
        &form,
        conduit_planner::SelectedRealizationPlanning {
            hosts: &advertisements,
            bases: &[BaseImplementationId::from("conduit.base/local@1")],
            requirements: &BTreeMap::new(),
            advertisements: &[realization],
            observations: &[observation],
            policies: &BTreeMap::new(),
            connection_item_capacity: 1,
            connection_byte_capacity: conduit_semantic_catalog::AUDIO_PLAY_ALSA_PCM_BLOCK_BYTES,
            authority_grants: &grants,
        },
    )
    .map_err(|error| format!("plan audio fixture: {error:?}"))?;
    Ok(plan.fragments[0].clone())
}

#[test]
fn exact_grant_and_resource_run_bounded_specimen_through_production_kernel() {
    let mut host = host(FakePlaybackBehavior::Success);
    let fragment = fragment(&host, true).unwrap();
    let playback = fragment
        .placements
        .iter()
        .find(|placement| placement.kind_id.as_str() == conduit_semantic_catalog::AUDIO_PLAY_KIND)
        .unwrap();
    assert_eq!(playback.resources.len(), 1);
    assert_eq!(playback.authority.len(), 1);
    assert!(!playback.resources[0].pool_id.as_str().contains("default"));
    assert_eq!(
        playback.execution_profile_id.as_str(),
        conduit_std_offers::AUDIO_PLAY_ALSA_HW_PROFILE
    );
    assert_eq!(fragment.connections.len(), 1);
    assert_eq!(fragment.connections[0].item_capacity, 1);
    assert_eq!(
        fragment.connections[0].byte_capacity,
        conduit_semantic_catalog::AUDIO_PLAY_ALSA_PCM_BLOCK_BYTES
    );

    let report = host
        .run_fragment_to(
            fragment,
            &mut Vec::with_capacity(2_048),
            &mut RecordingTimer { waits: Vec::new() },
        )
        .expect("fixture PCM runs through installed production kernel");
    assert!(matches!(
        report.observations.last().map(|item| &item.kind),
        Some(ObservationKind::PlanTerminal {
            disposition: TerminalDisposition::Completed
        })
    ));
    let kernel = report.kernel.unwrap();
    assert_eq!(kernel.playback.len(), 1);
    let playback = &kernel.playback[0];
    assert_eq!(playback.lifecycle, PlaybackLifecycle::StoppedClosed);
    assert_eq!(playback.metrics.blocks_committed, 96);
    assert_eq!(playback.metrics.frames_committed, 24_576);
    assert_eq!(playback.metrics.underruns, 0);
    assert_eq!(playback.backend, "deterministic-playback-fixture@1");
    assert_eq!(kernel.post_play_start_allocations, 0);
}

#[test]
fn discovery_offer_without_independent_authority_refuses_planning() {
    let host = host(FakePlaybackBehavior::Success);
    let error = fragment(&host, false).unwrap_err();
    assert!(error.contains("AuthorityGrantMissing"), "{error}");
}

#[test]
fn cancellation_before_open_never_commits_pcm_and_closes_the_session() {
    let mut host = host(FakePlaybackBehavior::Success);
    let fragment = fragment(&host, true).unwrap();
    let control = RunControl::default();
    control
        .request_stop(RunControlRequestId::new("cancel-before-audio-open").unwrap())
        .unwrap();
    let report = host
        .run_fragment_controlled_to(
            fragment,
            &mut Vec::with_capacity(2_048),
            &mut RecordingTimer { waits: Vec::new() },
            &control,
        )
        .expect("pre-open cancellation remains a successful control operation");
    assert!(matches!(
        report.observations.last().map(|item| &item.kind),
        Some(ObservationKind::PlanTerminal {
            disposition: TerminalDisposition::Cancelled { .. }
        })
    ));
    assert_eq!(report.control_receipts.len(), 1);
    let playback = &report.kernel.unwrap().playback[0];
    assert_eq!(playback.lifecycle, PlaybackLifecycle::StoppedClosed);
    assert_eq!(playback.metrics.blocks_committed, 0);
    assert_eq!(playback.metrics.frames_committed, 0);
}

#[test]
fn busy_open_underrun_provider_loss_and_drain_remain_distinct() {
    for (behavior, expected) in [
        (
            FakePlaybackBehavior::DeviceBusy,
            "OperationFailed(Failure { code: HostOperationDenied, detail: 71 })",
        ),
        (
            FakePlaybackBehavior::OpenFailure,
            "OperationFailed(Failure { code: HostOperationFailed, detail: 72 })",
        ),
        (
            FakePlaybackBehavior::UnderrunOnFirstBlock,
            "OperationFailed(Failure { code: HostOperationFailed, detail: 74 })",
        ),
        (
            FakePlaybackBehavior::ProviderLossOnFirstBlock,
            "OperationFailed(Failure { code: HostOperationFailed, detail: 75 })",
        ),
        (
            FakePlaybackBehavior::ProviderLossAfterFirstBlock,
            "OperationFailed(Failure { code: HostOperationFailed, detail: 75 })",
        ),
        (
            FakePlaybackBehavior::ProviderLossOnDrain,
            "OperationFailed(Failure { code: HostOperationFailed, detail: 75 })",
        ),
        (
            FakePlaybackBehavior::DrainFailure,
            "OperationFailed(Failure { code: HostOperationFailed, detail: 77 })",
        ),
    ] {
        let mut host = host(behavior);
        let fragment = fragment(&host, true).unwrap();
        let error = host
            .run_fragment_to(
                fragment,
                &mut Vec::with_capacity(2_048),
                &mut RecordingTimer { waits: Vec::new() },
            )
            .unwrap_err();
        assert!(error.contains(expected), "{behavior:?}: {error}");
    }
}

#[test]
fn stale_selection_and_wrong_grant_fail_before_play() {
    let config = StdHostConfig {
        host_id: HostId::from("stale-host"),
        boot_id: BootId::from("current-boot"),
        offer_generation: OfferGeneration(2),
    };
    let stale = HostedPlaybackSelection::deterministic_fake(
        AlsaPlaybackObservation {
            card_index: 0,
            card_id: "FIXTURE".into(),
            card_name: "fixture".into(),
            device: 0,
            device_name: "fixture".into(),
            base_identity: "fixture-base".into(),
        },
        BootId::from("old-boot"),
        config.offer_generation,
        FakePlaybackBehavior::Success,
    );
    assert!(StdHost::new_with_playback(config, StdHostComposition::reference(), stale).is_err());

    let host = host(FakePlaybackBehavior::Success);
    let mut grant = host.playback_authority_grant("grant/wrong").unwrap();
    grant.boot_id = BootId::from("wrong-boot");
    let form = form();
    let advertisements = [host.advertisement().clone()];
    let placements = conduit_planner::default_placements(&form, &advertisements).unwrap();
    let error = conduit_planner::plan_with_authority_grants(
        &form,
        &advertisements,
        &placements,
        &[BaseImplementationId::from("conduit.base/local@1")],
        &[grant],
    )
    .unwrap_err();
    assert!(matches!(
        error,
        conduit_planner::PlannerError::AuthorityGrantMissing(_)
    ));
}
