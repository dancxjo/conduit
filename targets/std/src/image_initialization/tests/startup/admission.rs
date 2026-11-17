use super::*;
use std::sync::atomic::Ordering;

#[test]
fn checked_speaker_refuses_stale_identity_and_cannot_reset_original_charge() {
    let metadata_host =
        super::super::host(&super::super::image(super::super::SOURCE), "boot/metadata").unwrap();
    let metadata = metadata_host
        .kernel_resources
        .image_memory_observer()
        .load(Ordering::Acquire);
    let (config, selected, checked, image) =
        fixture::equipment(FakePlaybackBehavior::Success, 67108864);
    let mut host = StdHost::from_selected_timer_and_playback_image(
        config.clone(),
        &image,
        &conduit_workspace_make::catalog(),
        &checked,
        &selected,
    )
    .unwrap();
    let counter = host.kernel_resources.image_memory_observer();
    let charged = counter.load(Ordering::Acquire);
    assert!(charged > metadata);
    assert!(host.attach_selected_playback(selected.clone()).is_err());
    assert_eq!(counter.load(Ordering::Acquire), charged);
    for case in 0..5 {
        let mut stale = selected.clone();
        match case {
            0 => stale.boot_id = "boot/stale".into(),
            1 => stale.offer_generation = OfferGeneration(2),
            2 => stale.observation.card_id = "Foreign".into(),
            3 => stale.observation.device = 1,
            _ => stale.observation.base_identity = "foreign-device".into(),
        }
        assert!(StdHost::from_selected_timer_and_playback_image(
            config.clone(),
            &image,
            &conduit_workspace_make::catalog(),
            &checked,
            &stale
        )
        .is_err());
    }
    drop(host);
    assert_eq!(counter.load(Ordering::Acquire), metadata);
}

#[test]
fn startup_one_byte_short_refuses_before_preparation_started_or_effect() {
    let (host, session) = fixture::fixture(FakePlaybackBehavior::Success);
    let proposed = session.realization().unwrap();
    let baseline = host
        .kernel_resources
        .image_memory_observer()
        .load(Ordering::Acquire);
    let control = crate::RunControl::default();
    let layout =
        crate::installed_std::body_kernel::funded::TickPreparationLayout::prepare_with_playback(
            &BodyRunRequest {
                wake: &proposed.wake,
                plan: &proposed.plan,
                control: &control,
                keyboard: None,
            },
            &host.advertisement,
            &host.kernel_resources,
            &crate::ThreadTimer,
            false,
            host.playback.as_ref(),
        )
        .unwrap();
    let required = baseline.checked_add(layout.peak_bytes).unwrap();
    for accepted in [true, false] {
        crate::hosted_audio::startup_chime::reset_test_attempts();
        let (config, selected, checked, image) = fixture::equipment(
            FakePlaybackBehavior::Success,
            required - usize::from(!accepted),
        );
        let host = StdHost::from_selected_timer_and_playback_image(
            config,
            &image,
            &conduit_workspace_make::catalog(),
            &checked,
            &selected,
        )
        .unwrap();
        let (mut host, mut session) = fixture::session_for_host(host);
        let proposed = session.realization().unwrap().clone();
        let ad = host.advertisement().clone();
        let counter = host.kernel_resources.image_memory_observer();
        let before = counter.load(Ordering::Acquire);
        let sequence = host.next_kernel_play_sequence;
        let mut started = false;
        let probe = crate::allocation_probe::begin();
        let result = host.run_selected_image_body_plan_with_committed_startup_to(
            BodyRunRequest {
                wake: &proposed.wake,
                plan: &proposed.plan,
                control: &control,
                keyboard: None,
            },
            &[],
            &mut Vec::new(),
            &mut crate::ThreadTimer,
            None,
            |play, wake| {
                started = true;
                session
                    .started(&ad.host_id, &ad.boot_id, play.clone(), wake.clone())
                    .unwrap();
                CommittedBodyStartup::from_retained(session.evidence(), &proposed.plan, play)
            },
        );
        let requested = probe.finish_bytes();
        assert_eq!(
            result.is_ok(),
            accepted,
            "{}",
            result.as_ref().err().map_or("accepted", String::as_str)
        );
        assert_eq!(started, accepted);
        if !accepted {
            assert!(
                requested < 128 * 1024,
                "refusal prepared oversized backing: {requested}"
            );
            assert_eq!(host.next_kernel_play_sequence, sequence);
            assert_eq!(crate::hosted_audio::startup_chime::test_attempts(), 0);
            assert_eq!(counter.load(Ordering::Acquire), before);
        }
        drop(result);
        assert_eq!(counter.load(Ordering::Acquire), before);
    }
}

#[test]
fn checked_ready_speaker_does_not_supply_playback_authority_to_planning() {
    let (config, selected, checked, image) =
        fixture::equipment(FakePlaybackBehavior::Success, 67_108_864);
    let host = StdHost::from_selected_timer_and_playback_image(
        config,
        &image,
        &conduit_workspace_make::catalog(),
        &checked,
        &selected,
    )
    .unwrap();
    let offer = host
        .advertisement()
        .capabilities
        .iter()
        .find(|offer| {
            offer.implementation.implementation_id.as_str()
                == conduit_std_offers::STARTUP_CHIME_IMPLEMENTATION
        })
        .unwrap();
    assert_eq!(offer.authority_requirements.len(), 1);
    let plot = fixture::expanded_cue();
    let hosts = [host.advertisement().clone()];
    let placements = conduit_planner::default_expanded_placements(&plot.expanded, &hosts).unwrap();
    let empty = std::collections::BTreeMap::new();
    let result = conduit_planner::plan_expanded_authoring_with_options(
        &plot,
        &hosts,
        &placements,
        &["conduit.base/local@1".into()],
        conduit_planner::PlanningOptions {
            connection_bases: &empty,
            line_candidates: &std::collections::BTreeMap::new(),
            connection_item_capacity: 1,
            connection_byte_capacity: conduit_core::BOOL_ENCODED_LEN as u32,
            authority_grants: &[],
            protected_resource_grants: &[],
            line_offers: &[],
        },
        &std::collections::BTreeMap::new(),
    );
    assert!(
        result.is_err(),
        "equipment readiness must not satisfy an independent grant requirement"
    );
}
