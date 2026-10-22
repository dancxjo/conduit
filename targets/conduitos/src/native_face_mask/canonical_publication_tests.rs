//! Actual native Mask admission/scanout ACK with the retained application owner.
//! CountingDisplay is CPU enactment, not installed/QEMU/physical proof.
use super::{CountingDisplay, fixture};
use alloc::{borrow::ToOwned, vec};
use conduit_body::{Body, BodyWorkset, ResidentPlot};
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
    #[derive(serde::Deserialize)]
    enum Fixture {
        Init {
            applications: [Option<Initial>; conduitos_application_core::SLOTS],
        },
    }
    let Fixture::Init { applications } = data::decode(include_bytes!(
        "../../application-core/tests/fixtures/startup.catz"
    ))
    .unwrap();
    let owner =
        crate::native_workset::resident(crate::native_workset::NativePlot::Patchbay).unwrap();
    let mut residents = vec![owner.clone()];
    if let Some(Initial::Patchbay { targets, .. }) = &applications[0] {
        for target in targets.iter().flatten() {
            let resident = ResidentPlot::new(
                target.graph.source_document_id.clone(),
                target.graph.checked_plot_id.clone(),
            );
            if !residents.contains(&resident) {
                residents.push(resident);
            }
        }
    }
    let body = Body::born_with_plots(
        BodyWorkset::from_plots(residents).unwrap(),
        1,
        "sign/canonical-patchbay-test".into(),
    )
    .unwrap();
    let mut models = Models::new(applications).unwrap();
    let graph = match models.execute(data::Command::Graph { slot: 0 }).unwrap() {
        Reply::Graph { graph: Some(graph) } => graph,
        _ => panic!("actual selected retained target"),
    };
    let subject = graph.subject_identities().next().unwrap().to_owned();
    let basis = FaceContributionBasis {
        checked_plot_id: owner.checked_plot_id.clone(),
        plan_id: "plan/canonical-patchbay-test".into(),
        active_play_id: "play/canonical-patchbay-test".into(),
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
        None,
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
    assert!(
        matches!(models.patchbay_publish(0,body.body_id.clone(),basis.clone(),face.clone(),show.clone()).unwrap(),
        Reply::PatchbayPublished {revision,..} if revision==before_revision)
    );
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
