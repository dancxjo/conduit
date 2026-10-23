//! Actual native Mask admission/scanout ACK with the retained application owner.
//! CountingDisplay is CPU enactment, not installed/QEMU/physical proof.
use super::{CountingDisplay, fixture};
use alloc::{borrow::ToOwned, vec};
use conduit_body::{Body, BodyBiographyEvidence, BodyMembership, BodyPlayIdentity, BodyWorkset};
use conduit_face::{
    BodyFace, FaceContext, FaceContribution, FaceContributionBasis, FaceContributionRole,
    FaceFocus, FaceInteraction, FaceInteractionArgument, ManifestationLifecycle,
};
use conduitos_application_core::{
    data::{self, CanonicalRefusal, Initial, Proposal, Reply},
    models::Models,
};
use patchbay_application::PatchbayApplicationRequest;

#[test]
fn acknowledged_native_show_admits_actual_owner_edit_once_and_rejects_republication() {
    // Use the actual native planner, kernel start, and lifecycle transitions.
    // The owner under test remains Models; no invented live Plan/Play witness.
    let native = crate::native_workset::inventory();
    let owner =
        crate::native_workset::resident(crate::native_workset::NativePlot::Patchbay).unwrap();
    let born = Body::born_with_plots(
        BodyWorkset::from_plots(
            native
                .into_iter()
                .map(|plot| crate::native_workset::resident(plot).unwrap()),
        )
        .unwrap(),
        1,
        "sign/canonical-patchbay-born".into(),
    )
    .unwrap();
    let membership = BodyMembership::new(born.body_id.clone()).unwrap();
    let mut evidence =
        BodyBiographyEvidence::born(born.clone(), membership, "Test Body".into()).unwrap();
    let (body, wake) = born.wake(2, "sign/canonical-patchbay-wake".into()).unwrap();
    evidence.append_wake(body.clone(), wake.clone(), 2).unwrap();
    let (ids, offer) = crate::native_workset::tests::fixture();
    let prepared = crate::native_workset::prepare(&wake, &ids, &offer, "build").unwrap();
    let active = BodyPlayIdentity::bind(prepared.plan(), 3);
    let mut play =
        crate::native_workset::NativeWorksetPlay::prepare_with_biography(&prepared, &evidence)
            .unwrap();
    play.start_for(prepared.plan(), &active).unwrap();
    let wake = wake
        .body_plan_ready(prepared.plan(), "sign/canonical-patchbay-plan".into())
        .unwrap()
        .body_play_started(
            prepared.plan(),
            &active,
            "sign/canonical-patchbay-play".into(),
        )
        .unwrap();
    let target = prepared
        .plan()
        .plots
        .iter()
        .find(|plot| plot.plot != owner)
        .unwrap();
    let expanded =
        crate::native_workset::checked(crate::native_workset::resolve(&target.plot).unwrap())
            .unwrap();
    let graph = patchbay_graph::PatchbayGraph::from_expanded(&expanded).unwrap();
    let mut applications = core::array::from_fn(|_| None);
    applications[0] = Some(Initial::Patchbay {
        targets: [
            Some(data::Target {
                graph,
                plan: target.plan.plan_id.clone(),
                body: prepared.plan().plan_id.clone(),
            }),
            None,
            None,
            None,
        ],
        selected: 0,
    });
    let mut models = Models::new(applications).unwrap();
    let graph = match models.execute(data::Command::Graph { slot: 0 }).unwrap() {
        Reply::Graph { graph: Some(graph) } => graph,
        _ => panic!("actual selected retained target"),
    };
    let subject = graph.subject_identities().next().unwrap().to_owned();
    let basis = FaceContributionBasis {
        checked_plot_id: owner.checked_plot_id.clone(),
        plan_id: prepared.plan().plan_id.clone(),
        active_play_id: active.active_play_id.clone(),
        required_interaction_context: None,
    };
    let (before_revision, fragment) = match models
        .patchbay_fragment(0, body.body_id.clone(), basis.clone(), true)
        .unwrap()
    {
        Reply::PatchbayFragment {
            revision, fragment, ..
        } => (revision, fragment),
        _ => panic!("actual selected actor fragment"),
    };
    let face = BodyFace::project(
        &body,
        Some(&wake),
        7,
        FaceContext::ResidentPlot(owner.checked_plot_id.clone()),
        FaceFocus::Contribution {
            role: FaceContributionRole::Foreground,
            node_key: None,
        },
        vec![FaceContribution::from_face(
            FaceContributionRole::Foreground,
            fragment,
        )],
    )
    .unwrap()
    .face;
    let (mut mask, _) = fixture();
    let mut display = CountingDisplay {
        writes: 0,
        fail: false,
    };
    mask.present(face.clone(), 1, 1, &mut display).unwrap();
    let show = mask.show().unwrap().clone();
    assert_eq!(show.show.lifecycle, ManifestationLifecycle::Available);
    assert!(display.writes > 0);
    let action = face
        .actions
        .iter()
        .find(|action| action.identity == patchbay_application::EDIT_CURRENT_ACTION_ID)
        .unwrap();
    let input = FaceInteraction::new(
        &face,
        &show,
        &action.identity,
        &action.target,
        vec![FaceInteractionArgument {
            name: "subject".into(),
            value_kind: action.arguments[0].contract.value_kind.as_str().into(),
            value: subject.as_bytes().to_vec(),
        }],
        1,
    )
    .unwrap();
    let publish = data::Command::PatchbayPublish {
        slot: 0,
        body: body.body_id.clone(),
        basis: basis.clone(),
        face: face.clone(),
        show: show.clone(),
    };
    let encoded = data::encode_command(&publish).unwrap();
    let mut workspace = data::Workspace::new().unwrap();
    let decoded = workspace.decode_command(&encoded).unwrap();
    assert!(matches!(models.execute(decoded).unwrap(),
        Reply::PatchbayPublished { revision, .. } if revision == before_revision));
    // Traverse the actual native Mask interaction validation before the typed
    // owner call; the actor is never fed a legacy ApplicationEvent conversion.
    let delivered = mask.submit(input).unwrap();
    match models
        .patchbay_consume(0, &delivered.interaction.encode())
        .unwrap()
    {
        Reply::PatchbayConsumed {
            revision,
            proposal:
                Some(Proposal::EditCurrent(PatchbayApplicationRequest::EditCurrent {
                    expanded_plot_id,
                    subject_identity,
                })),
            refusal: None,
            ..
        } => {
            assert_eq!(revision, before_revision + 1);
            assert_eq!(expanded_plot_id, graph.expanded_plot_id);
            assert_eq!(subject_identity, subject);
        }
        _ => panic!("actual owner emits exact typed Edit proposal"),
    }
    assert!(matches!(
        models
            .patchbay_consume(0, &delivered.interaction.encode())
            .unwrap(),
        Reply::PatchbayRefused {
            refusal: CanonicalRefusal::NoPublication,
            ..
        }
    ));
    assert!(matches!(
        models
            .patchbay_publish(0, body.body_id.clone(), basis.clone(), face, show)
            .unwrap(),
        Reply::PatchbayRefused {
            refusal: CanonicalRefusal::ShowConsumed,
            ..
        }
    ));
    match models
        .patchbay_fragment(0, body.body_id, basis, true)
        .unwrap()
    {
        Reply::PatchbayFragment { revision, .. } => assert_eq!(revision, before_revision + 1),
        _ => panic!("replay refusal retains the same authoritative owner"),
    }
}
