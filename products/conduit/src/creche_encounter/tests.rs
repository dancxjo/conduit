//! Canonical interaction/custody regression with explicit software Mask ACK.
//! The selected-show slot simulates the trusted renderer; no native output claim.
use super::*;
use conduit_body::{Body, BodyPlan, BodyPlayIdentity, BodyPlotPlan, ResidentPlot};
use conduit_core::{ConnectionTrack, PortDirection};
use conduit_face::FaceInteractionArgument;
use conduit_std_host::hosted_creche::CrecheHost;
use std::collections::BTreeMap;
#[path = "../../../../make/body/tests/common/deployment.rs"]
mod draft_fixture;
#[path = "../../../../make/body/tests/common/face_mask.rs"]
mod mask_fixture;

struct Custody(Arc<Mutex<Option<MaskShow>>>);
impl CrecheMaskShowCustody for Custody {
    fn validate_acknowledged(&mut self, _: &Face, show: &MaskShow) -> Result<(), String> {
        if self.0.lock().unwrap().as_ref() == Some(show) {
            Ok(())
        } else {
            Err("fixture selected renderer has not acknowledged this Show".into())
        }
    }
    fn retire_and_drain(&mut self) -> Result<(), String> {
        *self.0.lock().unwrap() = None;
        Ok(())
    }
}
fn adapter() -> (
    CrecheEncounterAdapter,
    CrecheEncounterController,
    Arc<Mutex<Option<MaskShow>>>,
) {
    let draft = draft_fixture::encounter("Garden");
    let profiles = draft
        .construction()
        .hosts()
        .iter()
        .map(|h| h.configuration.clone())
        .collect();
    let acknowledged = Arc::new(Mutex::new(None));
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
fn plan(
    adapter: &CrecheEncounterAdapter,
) -> (conduit_core::HostAdvertisement, BodyPlan, BodyPlayIdentity) {
    let mut host = conduit_std_host::StdHost::new().advertisement().clone();
    host.capabilities = vec![conduit_body_make::deployment_runtime_catalog::creche_std_offer()];
    host.resources = vec![adapter.resource_offer()];
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
    (host, plan, play)
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
fn act(
    owner: &mut CrecheHost<'_>,
    controller: &CrecheEncounterController,
    acknowledged: &Arc<Mutex<Option<MaskShow>>>,
    action: &str,
    value: Option<&[u8]>,
) {
    let face = controller.face().unwrap().unwrap();
    let show = mask_fixture::acknowledged_fixture_show(&face);
    *acknowledged.lock().unwrap() = Some(show.clone());
    controller.acknowledge_show(show.clone()).unwrap();
    controller
        .submit(interaction(&face, &show, action, value))
        .unwrap();
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
    *acknowledged.lock().unwrap() = Some(show.clone());
    controller.acknowledge_show(show.clone()).unwrap();
    let old = interaction(&original, &show, "creche.deploy.next", None);
    controller
        .submit(interaction(&original, &show, "creche.name", Some(b"Maple")))
        .unwrap();
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
    *acknowledged.lock().unwrap() = Some(show.clone());
    controller.acknowledge_show(show.clone()).unwrap();
    *acknowledged.lock().unwrap() = None;
    assert!(controller
        .submit(interaction(&face, &show, "creche.deploy.next", None))
        .is_err());
    *acknowledged.lock().unwrap() = Some(show.clone());
    controller
        .submit(interaction(&face, &show, "creche.deploy.next", None))
        .unwrap();
    *acknowledged.lock().unwrap() = None;
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
