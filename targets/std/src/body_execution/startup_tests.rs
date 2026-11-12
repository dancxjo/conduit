//! Existing Body lifecycle and production runner, with the explicit fake speaker.
use super::*;
use crate::hosted_audio::{AlsaPlaybackObservation, FakePlaybackBehavior, HostedPlaybackSelection};
use conduit_body::*;
use conduit_core::*;
use std::collections::BTreeMap;
struct Timer;
impl TimerAdapter for Timer {
    fn wait(&mut self, _: std::time::Duration) {}
}
fn fixture(behavior: FakePlaybackBehavior) -> (StdHost, BodyLifecycleSession) {
    let config = crate::StdHostConfig {
        host_id: "host/startup-fixture".into(),
        boot_id: "boot/startup-fixture".into(),
        offer_generation: OfferGeneration(1),
    };
    let mut host = StdHost::new_with_config(config.clone());
    let selected = HostedPlaybackSelection::deterministic_fake(
        AlsaPlaybackObservation {
            card_index: 999,
            card_id: "Fixture".into(),
            card_name: "Fixture".into(),
            device: 0,
            device_name: "PCM".into(),
            base_identity: "fixture/speaker".into(),
        },
        config.boot_id.clone(),
        config.offer_generation,
        behavior,
    )
    .with_executable_sha256("ab".repeat(32))
    .unwrap();
    host.attach_selected_playback(selected).unwrap();
    fixture_for_host(host)
}
fn fixture_for_host(host: StdHost) -> (StdHost, BodyLifecycleSession) {
    let config = crate::StdHostConfig {
        host_id: host.advertisement().host_id.clone(),
        boot_id: host.advertisement().boot_id.clone(),
        offer_generation: host.advertisement().offer_generation,
    };
    let (mut startup, mut profiles) = (
        conduit_plot::StartupCatalog::new(),
        conduit_plot::ProfileCatalog::new(),
    );
    conduit_semantic_catalog::install_body_startup_catalogs(&mut startup, &mut profiles).unwrap();
    let source = include_str!("../../../../plots/startup-chime/main.conduit");
    let checked =
        conduit_plot::check_syntax_document(&conduit_plot::parse_syntax_document(source), &startup)
            .unwrap();
    let plot =
        conduit_plot::expand_canonical_plot_for_authoring(&checked, "startup_chime", &profiles)
            .unwrap();
    let hosts = [host.advertisement().clone()];
    let placements = conduit_planner::default_expanded_placements(&plot.expanded, &hosts).unwrap();
    let grant = host
        .startup_chime_authority_grant("grant/fixture-explicit")
        .unwrap();
    let plan = conduit_planner::plan_expanded_authoring_with_options(
        &plot,
        &hosts,
        &placements,
        &["conduit.base/local@1".into()],
        conduit_planner::PlanningOptions {
            connection_bases: &BTreeMap::new(),
            line_candidates: &BTreeMap::new(),
            connection_item_capacity: 1,
            connection_byte_capacity: conduit_core::BOOL_ENCODED_LEN as u32,
            authority_grants: &[grant],
            protected_resource_grants: &[],
            line_offers: &[],
        },
        &BTreeMap::new(),
    )
    .unwrap();
    let resident = ResidentPlot::new(
        plan.source_document_id.clone(),
        plan.checked_plot_id.clone(),
    );
    let body = Body::born_with_plots(
        BodyWorkset::one(resident.clone()).unwrap(),
        1,
        "fixture/birth".into(),
    )
    .unwrap();
    let mut membership = BodyMembership::new(body.body_id.clone()).unwrap();
    let mut biography = BodyBiographyEvidence::born(
        body.clone(),
        membership.clone(),
        "Existing fixture Body".into(),
    )
    .unwrap();
    let part = PartId::bind(&body.body_id, config.host_id.as_str(), 0).unwrap();
    let proof = MembershipProofId::bind("fixture/explicit-member").unwrap();
    membership
        .admit(
            &body.body_id,
            membership.revision,
            part.clone(),
            proof.clone(),
            "fixture/admit".into(),
        )
        .unwrap();
    membership
        .observe_present(
            &body.body_id,
            membership.revision,
            &part,
            AuthenticatedHostObservation {
                host_id: config.host_id.clone(),
                boot_id: config.boot_id.clone(),
                offer_generation: config.offer_generation,
                proof_id: proof,
                sequence: 0,
            },
            "fixture/present".into(),
        )
        .unwrap();
    let events = membership
        .events
        .iter()
        .enumerate()
        .map(|(i, e)| (e.change_id.clone(), i as u64 + 2))
        .collect::<Vec<_>>();
    biography
        .append_membership_events(membership, &events)
        .unwrap();
    let mut session =
        BodyLifecycleSession::open_admitted(biography, &config.host_id, &config.boot_id).unwrap();
    session
        .propose(
            vec![BodyPlotPlan {
                plot: resident,
                plan,
            }],
            &config.host_id,
            &config.boot_id,
        )
        .unwrap();
    (host, session)
}
#[test]
fn cue_executes_in_the_existing_body_only_after_exact_retained_started_ack() {
    for (behavior, expected) in [
        (
            FakePlaybackBehavior::Success,
            conduit_kernel::HostCallDisposition::Completed,
        ),
        (
            FakePlaybackBehavior::DeviceBusy,
            conduit_kernel::HostCallDisposition::Denied,
        ),
        (
            FakePlaybackBehavior::OpenFailure,
            conduit_kernel::HostCallDisposition::Failed,
        ),
    ] {
        crate::hosted_audio::startup_chime::reset_test_attempts();
        let (mut host, mut session) = fixture(behavior);
        let existing_body = session.evidence().body_id.clone();
        let proposed = session.realization().unwrap().clone();
        let ad = host.advertisement().clone();
        let report = host
            .run_body_plan_with_committed_startup_to(
                BodyRunRequest {
                    wake: &proposed.wake,
                    plan: &proposed.plan,
                    control: &RunControl::default(),
                    keyboard: None,
                },
                &mut Vec::new(),
                &mut Timer,
                None,
                |play, wake| {
                    assert_eq!(play.body_id, existing_body);
                    assert!(CommittedBodyStartup::from_retained(
                        session.evidence(),
                        &proposed.plan,
                        play
                    )
                    .is_err());
                    session
                        .started(&ad.host_id, &ad.boot_id, play.clone(), wake.clone())
                        .unwrap();
                    CommittedBodyStartup::from_retained(session.evidence(), &proposed.plan, play)
                },
            )
            .unwrap();
        assert_eq!(report.terminal, TerminalDisposition::Completed);
        assert!(report.failure.is_none(), "{:?}", report.failure);
        assert_eq!(report.play.body_id, existing_body);
        assert_eq!(report.requests.len(), 1);
        assert_eq!(report.startup_chimes.len(), 1);
        let cue = &report.startup_chimes[0];
        assert_eq!(cue.disposition, Some(expected));
        assert!(cue.kernel_accepted);
        assert_eq!(crate::hosted_audio::startup_chime::test_attempts(), 1);
        let proof =
            CommittedBodyStartup::from_retained(session.evidence(), &proposed.plan, &report.play)
                .unwrap();
        assert!(proof.for_play(&proposed.plan, &report.play).is_ok());
        let next = BodyPlayIdentity::bind(&proposed.plan, report.play.play_sequence + 1);
        assert!(proof.for_play(&proposed.plan, &next).is_err());
        assert_eq!(
            cue.failure.is_some(),
            expected != conduit_kernel::HostCallDisposition::Completed
        );
    }
}
#[test]
fn lifecycle_only_or_failed_ack_cannot_fire_startup() {
    crate::hosted_audio::startup_chime::reset_test_attempts();
    let (mut host, session) = fixture(FakePlaybackBehavior::Success);
    let proposed = session.realization().unwrap().clone();
    let mut observed = false;
    let result = host.run_body_plan_to_with_start(
        BodyRunRequest {
            wake: &proposed.wake,
            plan: &proposed.plan,
            control: &RunControl::default(),
            keyboard: None,
        },
        &mut Vec::new(),
        &mut Timer,
        |_, _| {
            observed = true;
            Ok(())
        },
    );
    assert!(result.is_err());
    assert!(!observed, "refuse missing evidence before Started");
    assert_eq!(crate::hosted_audio::startup_chime::test_attempts(), 0);
    let (mut host, session) = fixture(FakePlaybackBehavior::Success);
    let proposed = session.realization().unwrap().clone();
    let result = host.run_body_plan_with_committed_startup_to(
        BodyRunRequest {
            wake: &proposed.wake,
            plan: &proposed.plan,
            control: &RunControl::default(),
            keyboard: None,
        },
        &mut Vec::new(),
        &mut Timer,
        None,
        |_, _| Err("retained Started failed".into()),
    );
    assert!(result.unwrap_err().contains("retained Started failed"));
    assert_eq!(crate::hosted_audio::startup_chime::test_attempts(), 0);
}

#[test]
fn image_chime_retains_original_memory_custody_through_execution_and_report() {
    use conduit_host_make::*;
    use std::sync::atomic::Ordering;
    crate::hosted_audio::startup_chime::reset_test_attempts();
    let (reference, _) = fixture(FakePlaybackBehavior::Success);
    let source = include_str!("../../profiles/raspberry-pi-3-tick-workset.host.conduit")
        .replace("  limits =", &format!(r#"  base = {{kind: "audio/speaker", implementation: "std/kernel-audio-play-alsa-hw@1", initialization: {{executable_sha256: "{}", card_id: "Fixture", device: 0, base_identity: "fixture/speaker"}}}}
  capability = {{kind: "sound/startup-chime", pinned_implementation: "std/kernel-startup-chime@1"}}
  capability = {{kind: "body/wake", pinned_implementation: "std/kernel-body-wake@1"}}
  need = {{id: "std/audio/alsa/fixture/speaker/card-Fixture/device-0", class: "conduit.resource/audio-playback-alsa-hw@1", slots: 1, bytes: 1}}
  limits ="#, "ab".repeat(32)));
    let catalog = conduit_workspace_make::catalog();
    let packages = conduit_workspace_make::package_set();
    let checked = check_host_configuration(
        parse_host_configuration_conduit(&source).unwrap(),
        &catalog,
        &packages,
    )
    .unwrap();
    let (image, _) = build_default_host_image(
        checked.profile().clone(),
        &catalog,
        &packages,
        &BuildInputs {
            source_identity: "fixture/chime-image".into(),
            toolchain_available: true,
        },
    )
    .unwrap();
    let config = crate::StdHostConfig {
        host_id: reference.advertisement().host_id.clone(),
        boot_id: reference.advertisement().boot_id.clone(),
        offer_generation: reference.advertisement().offer_generation,
    };
    let selected = reference.playback_selection().unwrap();
    let host = StdHost::from_selected_timer_and_playback_image(
        config.clone(),
        &image,
        &catalog,
        &checked,
        selected,
    )
    .unwrap();
    assert_eq!(
        host.advertisement()
            .bases
            .iter()
            .filter(|b| b.mechanism_family.as_str() == "audio/speaker")
            .count(),
        1
    );
    assert!(host.image_identity().is_some());
    let used = host.kernel_resources.image_memory_observer();
    let base_charge = used.load(Ordering::Acquire);
    let counter_bytes = crate::kernel_preparation::StagedImageMemory::counter_backing_bytes();
    assert!(base_charge > counter_bytes);
    let (mut host, mut session) = fixture_for_host(host);
    let proposed = session.realization().unwrap().clone();
    let ad = host.advertisement().clone();
    let report = host
        .run_selected_image_body_plan_with_committed_startup_to(
            BodyRunRequest {
                wake: &proposed.wake,
                plan: &proposed.plan,
                control: &RunControl::default(),
                keyboard: None,
            },
            &[],
            &mut Vec::new(),
            &mut crate::ThreadTimer,
            None,
            |play, wake| {
                session
                    .started(&ad.host_id, &ad.boot_id, play.clone(), wake.clone())
                    .unwrap();
                CommittedBodyStartup::from_retained(session.evidence(), &proposed.plan, play)
            },
        )
        .unwrap();
    assert_eq!(report.report().terminal, TerminalDisposition::Completed);
    assert_eq!(crate::hosted_audio::startup_chime::test_attempts(), 1);
    assert!(used.load(Ordering::Acquire) > base_charge);
    drop(report);
    assert_eq!(used.load(Ordering::Acquire), base_charge);
    drop(host);
    assert_eq!(used.load(Ordering::Acquire), counter_bytes); // retained counter metadata only
    let mut stale = selected.clone();
    stale.offer_generation = OfferGeneration(2);
    assert!(StdHost::from_selected_timer_and_playback_image(
        config.clone(),
        &image,
        &catalog,
        &checked,
        &stale
    )
    .is_err());
    // The same ordinary fabricated workset with a genuinely smaller admitted
    // heap must refuse before Started, without widening any Base or grant.
    let small = source.replace("heap_arena_bytes: 67108864", "heap_arena_bytes: 8192");
    let checked = check_host_configuration(
        parse_host_configuration_conduit(&small).unwrap(),
        &catalog,
        &packages,
    )
    .unwrap();
    let (image, _) = build_default_host_image(
        checked.profile().clone(),
        &catalog,
        &packages,
        &BuildInputs {
            source_identity: "fixture/chime-small-image".into(),
            toolchain_available: true,
        },
    )
    .unwrap();
    let host = StdHost::from_selected_timer_and_playback_image(
        config, &image, &catalog, &checked, selected,
    )
    .unwrap();
    let (mut host, session) = fixture_for_host(host);
    let proposed = session.realization().unwrap().clone();
    crate::hosted_audio::startup_chime::reset_test_attempts();
    let mut started = false;
    let result = host.run_selected_image_body_plan_with_committed_startup_to(
        BodyRunRequest {
            wake: &proposed.wake,
            plan: &proposed.plan,
            control: &RunControl::default(),
            keyboard: None,
        },
        &[],
        &mut Vec::new(),
        &mut crate::ThreadTimer,
        None,
        |_, _| {
            started = true;
            Err("should refuse before Started".into())
        },
    );
    let error = result.err().expect("undersized IMAGE must refuse");
    assert!(!started);
    assert!(
        error.contains("memory") || error.contains("heap"),
        "{error}"
    );
    assert_eq!(crate::hosted_audio::startup_chime::test_attempts(), 0);
}

#[test]
fn committed_startup_cancelled_before_execution_cannot_sound() {
    crate::hosted_audio::startup_chime::reset_test_attempts();
    let (mut host, mut session) = fixture(FakePlaybackBehavior::Success);
    let proposed = session.realization().unwrap().clone();
    let ad = host.advertisement().clone();
    let control = RunControl::default();
    let report = host
        .run_body_plan_with_committed_startup_to(
            BodyRunRequest {
                wake: &proposed.wake,
                plan: &proposed.plan,
                control: &control,
                keyboard: None,
            },
            &mut Vec::new(),
            &mut Timer,
            None,
            |play, wake| {
                session
                    .started(&ad.host_id, &ad.boot_id, play.clone(), wake.clone())
                    .unwrap();
                let proof =
                    CommittedBodyStartup::from_retained(session.evidence(), &proposed.plan, play)?;
                control
                    .request_stop(
                        crate::RunControlRequestId::new("fixture/cancel-after-commit").unwrap(),
                    )
                    .unwrap();
                Ok(proof)
            },
        )
        .unwrap();
    assert!(matches!(
        report.terminal,
        TerminalDisposition::Cancelled { .. }
    ));
    assert_eq!(crate::hosted_audio::startup_chime::test_attempts(), 0);
    assert!(report.startup_chimes.iter().all(|r| r.request.is_none()));
}

#[test]
fn startup_cancelled_after_first_pcm_block_retires_the_exact_pending_request() {
    crate::hosted_audio::startup_chime::reset_test_attempts();
    let (mut host, mut session) = fixture(FakePlaybackBehavior::Success);
    let proposed = session.realization().unwrap().clone();
    let ad = host.advertisement().clone();
    let control = RunControl::default();
    crate::hosted_audio::startup_chime::cancel_after_first_test_block();
    let report = host
        .run_body_plan_with_committed_startup_to(
            BodyRunRequest {
                wake: &proposed.wake,
                plan: &proposed.plan,
                control: &control,
                keyboard: None,
            },
            &mut Vec::new(),
            &mut Timer,
            None,
            |play, wake| {
                session
                    .started(&ad.host_id, &ad.boot_id, play.clone(), wake.clone())
                    .unwrap();
                CommittedBodyStartup::from_retained(session.evidence(), &proposed.plan, play)
            },
        )
        .unwrap();
    assert!(
        matches!(report.terminal, TerminalDisposition::Cancelled { .. }),
        "terminal={:?} failure={:?} cue={:?}",
        report.terminal,
        report.failure,
        report.startup_chimes
    );
    assert!(report.failure.is_none(), "{:?}", report.failure);
    assert_eq!(crate::hosted_audio::startup_chime::test_attempts(), 1);
    let receipt = &report.startup_chimes[0];
    assert!(receipt.request.is_some());
    assert_eq!(
        receipt.disposition,
        Some(conduit_kernel::HostCallDisposition::Cancelled)
    );
    assert_eq!(
        receipt.failure.unwrap().code,
        conduit_kernel::FailureCode::Cancelled
    );
    // Global scheduler cancellation retires pending calls without accepting a
    // normal completion. Keep the physical outcome and exact RunCancelled Sign.
    assert!(!receipt.kernel_accepted);
    assert!(report
        .kernel_events
        .iter()
        .any(|e| e.kind == conduit_kernel::KernelEventKind::RunCancelled));
    assert!(!report.kernel_events.iter().any(|e| e.node == receipt.node
        && e.request == receipt.request
        && e.kind == conduit_kernel::KernelEventKind::HostCallCompleted));
}

#[test]
fn remote_startup_requires_original_body_fragment_and_retained_start_before_stepping() {
    use conduit_body::PreparedBodyFragment;
    use conduit_kernel::scheduler::SchedulerStatus;
    crate::hosted_audio::startup_chime::reset_test_attempts();
    let (mut host, mut session) = fixture(FakePlaybackBehavior::Success);
    let proposed = session.realization().unwrap().clone();
    let fragment = &proposed.plan.plots[0].plan.fragments[0];
    let play = host
        .prospective_body_play(&proposed.wake, &proposed.plan)
        .unwrap();
    let ad = host.advertisement().clone();
    let mut remote = host.prepare_remote_fragment(fragment).unwrap();
    assert!(remote.step().unwrap_err().contains("retained Started"));
    assert!(remote.next_host_request().is_none());
    assert!(
        CommittedBodyStartup::from_retained(session.evidence(), &proposed.plan, &play).is_err()
    );
    let control = RunControl::default();
    let request = BodyRunRequest {
        wake: &proposed.wake,
        plan: &proposed.plan,
        control: &control,
        keyboard: None,
    };
    let (started_wake, _) = super::image::prepare_lifecycle(&request, &play, &ad).unwrap();
    session
        .started(&ad.host_id, &ad.boot_id, play.clone(), started_wake)
        .unwrap();
    let committed =
        CommittedBodyStartup::from_retained(session.evidence(), &proposed.plan, &play).unwrap();
    let wrong_play = BodyPlayIdentity::bind(&proposed.plan, play.play_sequence + 1);
    let wrong =
        PreparedBodyFragment::prepare(&proposed.wake, &proposed.plan, &wrong_play, fragment)
            .unwrap();
    assert!(remote
        .bind_committed_body_startup(wrong, &committed)
        .is_err());
    assert!(remote.step().is_err());
    let scope =
        PreparedBodyFragment::prepare(&proposed.wake, &proposed.plan, &play, fragment).unwrap();
    remote
        .bind_committed_body_startup(scope, &committed)
        .unwrap();
    assert!(remote
        .bind_committed_body_startup(scope, &committed)
        .is_err());
    let mut pending = None;
    for _ in 0..8 {
        assert!(!matches!(
            remote.step().unwrap(),
            SchedulerStatus::Cancelled
        ));
        if let Some(request) = remote.next_host_request() {
            pending = Some(request);
            break;
        }
    }
    let pending = pending.expect("bound original source must reach the exact cue request");
    assert_eq!(
        remote
            .describe_host_request(pending)
            .unwrap()
            .contract_id
            .as_str(),
        conduit_std_offers::STARTUP_CHIME_OPERATION
    );
    assert_eq!(
        crate::hosted_audio::startup_chime::test_attempts(),
        0,
        "binding is not output dispatch"
    );
    remote.cancel().unwrap();
    assert!(matches!(remote.step().unwrap(), SchedulerStatus::Cancelled));
    host.release_remote_fragment(remote).unwrap();
    assert!(host.kernel_resources.is_idle());
    let mut unstarted = host.prepare_remote_fragment(fragment).unwrap();
    unstarted.cancel().unwrap();
    assert!(matches!(
        unstarted.step().unwrap(),
        SchedulerStatus::Cancelled
    ));
    assert!(unstarted
        .bind_committed_body_startup(scope, &committed)
        .is_err());
    host.release_remote_fragment(unstarted).unwrap();
    assert!(host.kernel_resources.is_idle());
}

fn started_remote_cue(behavior: FakePlaybackBehavior) -> (StdHost, crate::AdmittedRemoteFragment) {
    let (mut host, mut session) = fixture(behavior);
    let proposed = session.realization().unwrap().clone();
    let fragment = &proposed.plan.plots[0].plan.fragments[0];
    let play = host
        .prospective_body_play(&proposed.wake, &proposed.plan)
        .unwrap();
    let ad = host.advertisement().clone();
    let mut remote = host.prepare_remote_fragment(fragment).unwrap();
    assert!(
        remote.retained_startup_profile_backing_bytes().unwrap()
            > crate::hosted_audio::startup_chime::PCM_BYTES
    );
    let control = RunControl::default();
    let request = BodyRunRequest {
        wake: &proposed.wake,
        plan: &proposed.plan,
        control: &control,
        keyboard: None,
    };
    let (wake, _) = super::image::prepare_lifecycle(&request, &play, &ad).unwrap();
    session
        .started(&ad.host_id, &ad.boot_id, play.clone(), wake)
        .unwrap();
    let committed =
        CommittedBodyStartup::from_retained(session.evidence(), &proposed.plan, &play).unwrap();
    let scope = conduit_body::PreparedBodyFragment::prepare(
        &proposed.wake,
        &proposed.plan,
        &play,
        fragment,
    )
    .unwrap();
    remote
        .bind_committed_body_startup(scope, &committed)
        .unwrap();
    assert!(remote.retained_startup_profile_backing_bytes().is_err());
    (host, remote)
}
fn next_remote_cue(
    remote: &mut crate::AdmittedRemoteFragment,
) -> conduit_kernel::scheduler::HostCallRequest {
    for _ in 0..8 {
        remote.step().unwrap();
        if let Some(request) = remote.next_host_request() {
            return request;
        }
    }
    panic!("expected original pending cue");
}
#[test]
fn remote_cue_dispatch_uses_the_original_pending_request_and_retains_physical_outcome() {
    use conduit_kernel::HostCallDisposition as D;
    for (behavior, expected) in [
        (FakePlaybackBehavior::Success, D::Completed),
        (FakePlaybackBehavior::DeviceBusy, D::Denied),
        (FakePlaybackBehavior::OpenFailure, D::Failed),
    ] {
        crate::hosted_audio::startup_chime::reset_test_attempts();
        let (mut host, mut remote) = started_remote_cue(behavior);
        let request = next_remote_cue(&mut remote);
        let mut forged = request;
        forged.request.0 += 1;
        assert!(host
            .complete_remote_startup_host_call(&mut remote, forged, || false)
            .is_err());
        assert_eq!(crate::hosted_audio::startup_chime::test_attempts(), 0);
        assert!(host
            .complete_remote_startup_host_call(&mut remote, request, || false)
            .unwrap());
        assert!(host
            .complete_remote_startup_host_call(&mut remote, request, || false)
            .is_err());
        assert_eq!(crate::hosted_audio::startup_chime::test_attempts(), 1);
        let receipt = remote.startup_cue_receipts().next().unwrap();
        assert_eq!(receipt.disposition, Some(expected));
        assert!(receipt.kernel_accepted);
        assert_eq!(receipt.failure.is_some(), expected != D::Completed);
        let mut drained = false;
        for _ in 0..8 {
            if remote.step().unwrap() == conduit_kernel::scheduler::SchedulerStatus::Drained {
                drained = true;
                break;
            }
        }
        assert!(drained);
        host.release_remote_fragment(remote).unwrap();
        assert!(host.kernel_resources.is_idle());
    }
}
#[test]
fn remote_cue_cancellation_retires_without_a_normal_completion_or_second_effect() {
    crate::hosted_audio::startup_chime::reset_test_attempts();
    let (mut host, mut remote) = started_remote_cue(FakePlaybackBehavior::Success);
    let request = next_remote_cue(&mut remote);
    assert!(host
        .complete_remote_startup_host_call(&mut remote, request, || true)
        .unwrap());
    let receipt = remote.startup_cue_receipts().next().unwrap();
    assert_eq!(
        receipt.disposition,
        Some(conduit_kernel::HostCallDisposition::Cancelled)
    );
    assert!(!receipt.kernel_accepted);
    assert_eq!(crate::hosted_audio::startup_chime::test_attempts(), 0);
    assert!(host
        .complete_remote_startup_host_call(&mut remote, request, || false)
        .is_err());
    assert_eq!(
        remote.step().unwrap(),
        conduit_kernel::scheduler::SchedulerStatus::Cancelled
    );
    host.release_remote_fragment(remote).unwrap();
    assert!(host.kernel_resources.is_idle());
}
