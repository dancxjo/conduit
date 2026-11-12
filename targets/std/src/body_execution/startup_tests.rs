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
