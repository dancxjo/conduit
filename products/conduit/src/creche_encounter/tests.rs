//! Canonical interaction/custody regression with explicit software Mask ACK.
//! The selected-show slot simulates the trusted renderer; no native output claim.
use super::*;
use crate::creche_draft_fixture as draft_fixture;
use crate::creche_mask_fixture as mask_fixture;
use conduit_body::{Body, BodyPlan, BodyPlayIdentity, BodyPlotPlan, ResidentPlot};
use conduit_core::{ConnectionTrack, PortDirection};
use conduit_face::FaceInteractionArgument;
use conduit_std_host::hosted_creche::CrecheHost;
use std::collections::BTreeMap;

#[derive(Default)]
pub(super) struct FixtureReturnedInput {
    show: Option<MaskShow>,
    returned: Option<FaceInteraction>,
}
impl FixtureReturnedInput {
    pub(super) fn has_show(&self) -> bool {
        self.show.is_some()
    }
}
struct Custody(Arc<Mutex<FixtureReturnedInput>>);
impl CrecheMaskShowCustody for Custody {
    fn validate_acknowledged(&mut self, _: &Face, show: &MaskShow) -> Result<(), String> {
        if self.0.lock().unwrap().show.as_ref() == Some(show) {
            Ok(())
        } else {
            Err("fixture selected renderer has not acknowledged this Show".into())
        }
    }
    fn validate_returned_interaction(
        &mut self,
        _: &Face,
        _: &MaskShow,
        interaction: &FaceInteraction,
    ) -> Result<(), String> {
        if self.0.lock().unwrap().returned.as_ref() == Some(interaction) {
            Ok(())
        } else {
            Err("fixture Mask did not return this exact input occurrence".into())
        }
    }
    fn retire_and_drain(&mut self) -> Result<(), String> {
        *self.0.lock().unwrap() = FixtureReturnedInput::default();
        Ok(())
    }
}
pub(super) fn adapter() -> (
    CrecheEncounterAdapter,
    CrecheEncounterController,
    Arc<Mutex<FixtureReturnedInput>>,
) {
    let draft = draft_fixture::encounter("Garden");
    let profiles = draft
        .construction()
        .hosts()
        .iter()
        .map(|h| h.configuration.clone())
        .collect();
    let acknowledged = Arc::new(Mutex::new(FixtureReturnedInput::default()));
    let (adapter, controller) = CrecheEncounterAdapter::new(
        draft,
        profiles,
        "fixture/deployment".into(),
        "fixture/creche-slot".into(),
        CrecheLimits {
            maximum_evidence_bytes: 4096,
            maximum_polls: 128,
            minimum_poll_interval_ms: 1,
        },
        32,
        Box::new(Custody(acknowledged.clone())),
    )
    .unwrap();
    (adapter, controller, acknowledged)
}
pub(crate) fn plan(
    adapter: &CrecheEncounterAdapter,
) -> (conduit_core::HostAdvertisement, BodyPlan, BodyPlayIdentity) {
    plan_on_host(
        adapter,
        conduit_std_host::StdHost::new().advertisement().clone(),
    )
}
pub(crate) fn plan_on_host(
    adapter: &CrecheEncounterAdapter,
    host: conduit_core::HostAdvertisement,
) -> (conduit_core::HostAdvertisement, BodyPlan, BodyPlayIdentity) {
    let (host, _, plan, play) = plan_and_wake_on_host(adapter, host);
    (host, plan, play)
}
pub(crate) fn plan_and_wake_on_host(
    adapter: &CrecheEncounterAdapter,
    mut host: conduit_core::HostAdvertisement,
) -> (
    conduit_core::HostAdvertisement,
    conduit_body::Wake,
    BodyPlan,
    BodyPlayIdentity,
) {
    let capability = conduit_body_make::deployment_runtime_catalog::creche_std_offer();
    if !host
        .capabilities
        .iter()
        .any(|offer| offer.capability_id == capability.capability_id)
    {
        host.capabilities.push(capability);
    }
    let resource = adapter.resource_offer();
    if !host
        .resources
        .iter()
        .any(|offer| offer.pool_id == resource.pool_id)
    {
        host.resources.push(resource);
    }
    host.resources.sort();
    host.capabilities.sort_by(|a, b| a.capability_id.cmp(&b.capability_id));
    let mut startup = conduit_plot::StartupCatalog::new();
    let mut profile = conduit_plot::ProfileCatalog::new();
    conduit_body_make::deployment_runtime_catalog::install_creche_catalogs(
        &mut startup,
        &mut profile,
    )
    .unwrap();
    let checked = conduit_plot::check_syntax_document(
        &conduit_plot::parse_syntax_document(include_str!("../../../../plots/creche/main.conduit")),
        &startup,
    )
    .unwrap();
    let expanded =
        conduit_plot::expand_canonical_plot_for_authoring(&checked, "creche/main", &profile)
            .unwrap();
    let hosts = [host.clone()];
    let placements =
        conduit_planner::default_expanded_placements(&expanded.expanded, &hosts).unwrap();
    let boundaries = [
        (PortDirection::Input, "request", 32),
        (PortDirection::Output, "outcome", 65),
    ]
    .into_iter()
    .map(|(direction, name, bytes)| {
        (
            conduit_planner::ForeBoundaryKey {
                direction,
                front_port_id: conduit_core::port_id(name),
                track: ConnectionTrack::Payload,
            },
            conduit_planner::ConnectionQueueLimits {
                item_capacity: 1,
                byte_capacity: bytes,
            },
        )
    })
    .collect();
    let plan = conduit_planner::plan_expanded_authoring_with_options(
        &expanded,
        &hosts,
        &placements,
        &["conduit.base/local@1".into()],
        conduit_planner::PlanningOptions {
            connection_bases: &BTreeMap::new(),
            line_candidates: &BTreeMap::new(),
            connection_item_capacity: 1,
            connection_byte_capacity: 65,
            authority_grants: &[],
            protected_resource_grants: &[],
            line_offers: &[],
        },
        &boundaries,
    )
    .unwrap();
    let body = Body::born(
        plan.source_document_id.clone(),
        plan.checked_plot_id.clone(),
        1,
        "fixture/creche-born".into(),
    )
    .unwrap();
    let wake = body.wake(1, "fixture/creche-wake".into()).unwrap().1;
    let plan = BodyPlan::seal(
        &wake,
        vec![BodyPlotPlan {
            plot: ResidentPlot::new(
                plan.source_document_id.clone(),
                plan.checked_plot_id.clone(),
            ),
            plan,
        }],
    )
    .unwrap();
    let play = BodyPlayIdentity::bind(&plan, 0);
    (host, wake, plan, play)
}
fn interaction(
    face: &Face,
    show: &MaskShow,
    action: &str,
    value: Option<&[u8]>,
) -> FaceInteraction {
    let action = face
        .actions
        .iter()
        .find(|a| a.identity == action)
        .unwrap_or_else(|| {
            panic!(
                "action {action} is absent from current Face revision {}",
                face.revision
            )
        });
    let arguments = value
        .map(|value| {
            vec![FaceInteractionArgument {
                name: action.arguments[0].name.clone(),
                value_kind: action.arguments[0].contract.value_kind.as_str().into(),
                value: value.to_vec(),
            }]
        })
        .unwrap_or_default();
    FaceInteraction::new(face, show, &action.identity, &action.target, arguments, 1).unwrap()
}
pub(super) fn act(
    owner: &mut CrecheHost<'_>,
    controller: &CrecheEncounterController,
    acknowledged: &Arc<Mutex<FixtureReturnedInput>>,
    action: &str,
    value: Option<&[u8]>,
) {
    let face = controller.face().unwrap().unwrap();
    let show = mask_fixture::acknowledged_fixture_show(&face);
    acknowledged.lock().unwrap().show = Some(show.clone());
    controller.acknowledge_show(show.clone()).unwrap();
    let returned = interaction(&face, &show, action, value);
    acknowledged.lock().unwrap().returned = Some(returned.clone());
    controller.submit(returned).unwrap();
    std::thread::sleep(owner.poll_wait());
    assert_eq!(owner.poll().unwrap(), CrechePoll::Pending);
}

#[test]
fn encoded_available_show_without_selected_renderer_custody_is_rejected() {
    let (mut adapter, controller, _) = adapter();
    let (host, plan, play) = plan(&adapter);
    let mut owner = CrecheHost::prepare(&mut adapter, &host, &plan, &play).unwrap();
    assert!(controller.face().unwrap().is_none());
    owner.confirm_started(&host, &play).unwrap();
    owner.begin(owner.request()).unwrap();
    let face = controller.face().unwrap().unwrap();
    let show = mask_fixture::acknowledged_fixture_show(&face);
    assert!(controller.acknowledge_show(show.clone()).is_err());
    assert!(controller
        .submit(interaction(&face, &show, "creche.deploy.next", None))
        .is_err());
    owner.retire().unwrap();
}

#[test]
fn canonical_edits_and_one_review_handoff_do_not_claim_successful_images() {
    let (mut adapter, controller, acknowledged) = adapter();
    let (host, plan, play) = plan(&adapter);
    let mut owner = CrecheHost::prepare(&mut adapter, &host, &plan, &play).unwrap();
    owner.confirm_started(&host, &play).unwrap();
    owner.begin(owner.request()).unwrap();
    for (action, value) in [
        ("creche.name", Some(b"Willow".as_slice())),
        ("creche.deploy.next", None),
        ("creche.deploy.host-name.0", Some(b"seedling".as_slice())),
        ("creche.deploy.scope", Some(b"all".as_slice())),
        ("creche.deploy.next", None),
        ("creche.deploy.source", Some(b"build".as_slice())),
        ("creche.deploy.review", None),
    ] {
        act(&mut owner, &controller, &acknowledged, action, value);
    }
    assert!(controller.reviewed().unwrap());
    std::thread::sleep(owner.poll_wait());
    assert_eq!(owner.poll().unwrap(), CrechePoll::Pending);
    let handoff = controller.take_reviewed().unwrap().unwrap();
    assert_eq!(handoff.request, owner.request());
    assert!(controller.take_reviewed().unwrap().is_none());
    let prepared = handoff
        .draft
        .prepare(
            handoff.review,
            1,
            "fixture/new-body-birth".into(),
            &conduit_workspace_make::catalog(),
            &conduit_workspace_make::package_set(),
        )
        .unwrap();
    assert_eq!(prepared.birth().evidence().friendly_name, "Willow");
    assert_eq!(prepared.first_host(), "seedling");
    assert_eq!(prepared.selected_hosts(), ["second", "seedling"]);
    controller
        .classify_make_refusal(CrecheMakeRefusal::UnsupportedTarget)
        .unwrap();
    std::thread::sleep(owner.poll_wait());
    let CrechePoll::Outcome(outcome) = owner.poll().unwrap() else {
        panic!("exact classified refusal")
    };
    assert_eq!(outcome.disposition, CrecheDisposition::Refused);
    owner.retire().unwrap();
}

#[test]
fn mailbox_pressure_stale_show_and_revoked_renderer_custody_cannot_edit_the_model() {
    let (mut adapter, controller, acknowledged) = adapter();
    let (host, plan, play) = plan(&adapter);
    let mut owner = CrecheHost::prepare(&mut adapter, &host, &plan, &play).unwrap();
    owner.confirm_started(&host, &play).unwrap();
    owner.begin(owner.request()).unwrap();
    let original = controller.face().unwrap().unwrap();
    let show = mask_fixture::acknowledged_fixture_show(&original);
    acknowledged.lock().unwrap().show = Some(show.clone());
    controller.acknowledge_show(show.clone()).unwrap();
    let old = interaction(&original, &show, "creche.deploy.next", None);
    let returned = interaction(&original, &show, "creche.name", Some(b"Maple"));
    acknowledged.lock().unwrap().returned = Some(returned.clone());
    controller.submit(returned).unwrap();
    assert!(controller.submit(old.clone()).is_err());
    std::thread::sleep(owner.poll_wait());
    owner.poll().unwrap();
    assert_ne!(
        controller.face().unwrap().unwrap().identity,
        original.identity
    );
    assert!(controller.acknowledge_show(show).is_err());
    assert!(controller.submit(old).is_err());
    let face = controller.face().unwrap().unwrap();
    let show = mask_fixture::acknowledged_fixture_show(&face);
    acknowledged.lock().unwrap().show = Some(show.clone());
    controller.acknowledge_show(show.clone()).unwrap();
    acknowledged.lock().unwrap().show = None;
    assert!(controller
        .submit(interaction(&face, &show, "creche.deploy.next", None))
        .is_err());
    acknowledged.lock().unwrap().show = Some(show.clone());
    let returned = interaction(&face, &show, "creche.deploy.next", None);
    acknowledged.lock().unwrap().returned = Some(returned.clone());
    controller.submit(returned).unwrap();
    acknowledged.lock().unwrap().show = None;
    std::thread::sleep(owner.poll_wait());
    let CrechePoll::Outcome(refused) = owner.poll().unwrap() else {
        panic!("revoked selected renderer refuses queued input")
    };
    assert_eq!(refused.disposition, CrecheDisposition::Refused);
    assert_eq!(
        controller
            .channel
            .lock()
            .unwrap()
            .face
            .as_ref()
            .unwrap()
            .identity,
        face.identity
    );
    owner.retire().unwrap();
    assert!(controller.face().unwrap().is_none());
}

#[test]
fn acknowledged_show_does_not_admit_fabricated_or_revoked_returned_input() {
    let (mut adapter, controller, custody) = adapter();
    let (host, plan, play) = plan(&adapter);
    let mut owner = CrecheHost::prepare(&mut adapter, &host, &plan, &play).unwrap();
    owner.confirm_started(&host, &play).unwrap();
    owner.begin(owner.request()).unwrap();
    let face = controller.face().unwrap().unwrap();
    let show = mask_fixture::acknowledged_fixture_show(&face);
    custody.lock().unwrap().show = Some(show.clone());
    controller.acknowledge_show(show.clone()).unwrap();
    let fabricated = interaction(&face, &show, "creche.name", Some(b"Fabricated"));
    assert!(
        controller.submit(fabricated.clone()).is_err(),
        "Show custody is not returned input custody"
    );
    let returned = interaction(&face, &show, "creche.name", Some(b"Returned"));
    // Explicitly simulate the fixture Mask's typed return, independently of ACK.
    custody.lock().unwrap().returned = Some(returned.clone());
    assert!(
        controller.submit(fabricated).is_err(),
        "another same-Show occurrence was never returned"
    );
    controller.submit(returned).unwrap();
    custody.lock().unwrap().returned = None;
    std::thread::sleep(owner.poll_wait());
    let CrechePoll::Outcome(outcome) = owner.poll().unwrap() else {
        panic!("queued input lost its returned-fore custody")
    };
    assert_eq!(outcome.disposition, CrecheDisposition::Refused);
    assert_eq!(
        controller
            .channel
            .lock()
            .unwrap()
            .face
            .as_ref()
            .unwrap()
            .identity,
        face.identity
    );
    owner.retire().unwrap();
}

#[test]
fn ui_ack_and_submit_report_custody_pressure_without_blocking_renderer_retirement() {
    let (mut adapter, controller, selected) = adapter();
    let (host, plan, play) = plan(&adapter);
    let mut owner = CrecheHost::prepare(&mut adapter, &host, &plan, &play).unwrap();
    owner.confirm_started(&host, &play).unwrap();
    owner.begin(owner.request()).unwrap();
    let face = controller.face().unwrap().unwrap();
    let show = mask_fixture::acknowledged_fixture_show(&face);
    selected.lock().unwrap().show = Some(show.clone());
    controller.acknowledge_show(show.clone()).unwrap();
    let returned = interaction(&face, &show, "creche.name", Some(b"Returned"));
    selected.lock().unwrap().returned = Some(returned.clone());
    for submit in [false, true] {
        // Simulate the worker holding custody while it awaits main-thread drain.
        let guard = controller.channel.custody.lock().unwrap();
        let ui = controller.clone();
        let (sent, received) = std::sync::mpsc::sync_channel(1);
        let show = show.clone();
        let returned = returned.clone();
        let worker = std::thread::spawn(move || {
            let result = if submit {
                ui.submit(returned)
            } else {
                ui.acknowledge_show(show)
            };
            sent.send(result).unwrap();
        });
        let result = received.recv_timeout(std::time::Duration::from_millis(250));
        // Always release before assertions so a regression cannot strand a worker.
        drop(guard);
        worker.join().unwrap();
        assert!(
            matches!(result, Ok(Err(ref message)) if message.contains("busy")),
            "UI blocked instead of observing selected renderer pressure: {result:?}"
        );
    }
    owner.retire().unwrap();
}
