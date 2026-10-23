//! CPU Root/actual actor integration with substituted decoded service returns.
//! Valid MaskShow is a fixture, not live scanout or hostile guest execution.
use super::*;
use conduit_body::BodyId;
use conduit_core::{ActivePlayId, ExpandedPlotId, SourceDocumentId};
use conduit_face::*;
use patchbay_application::PatchbayApplicationRequest;
use transport::{counts, reset, substitute};

// The shared Face fixture retains its owning Rust edition.
#[rustfmt::skip]
#[path = "../../../../semantics/face/tests/common/mod.rs"]
mod mask_fixture;

fn graph(plot: conduitos::native_workset::NativePlot) -> PatchbayGraph {
    let expanded = conduitos::native_workset::checked(plot).unwrap();
    PatchbayGraph::from_expanded(&expanded).unwrap()
}
struct Encounter {
    owner: Owner,
    face: Face,
    show: MaskShow,
    graphs: [PatchbayGraph; 2],
}
fn encounter() -> Encounter {
    reset();
    // Include all existing broker entries in CPU compilation without executing
    // unrelated legacy/Tour/cold projections or claiming their enactment.
    let _ = (
        Owner::apply,
        Owner::refresh,
        Owner::select,
        Owner::topology,
        Owner::complete,
        Owner::project_graph,
        Owner::project_tutorial,
        project_graph,
        project_tutorial,
    );
    let graphs = [
        graph(conduitos::native_workset::NativePlot::KeyboardCanvas),
        graph(conduitos::native_workset::NativePlot::MemoryLantern),
    ];
    let mut targets = core::array::from_fn(|_| None);
    for (slot, graph) in [1, 3].into_iter().zip(&graphs) {
        targets[slot] = Some(Target {
            graph: graph.clone(),
            plan: Some(alloc::format!("plan/target-{slot}").into()),
            body: "plan/body".into(),
        });
    }
    let mut owner = Owner::new([
        Some(Initial::Patchbay {
            targets,
            selected: 1,
        }),
        None,
        None,
        None,
    ])
    .unwrap();
    owner.canonical(0).unwrap();
    let body: BodyId = serde_json::from_str("\"body/root-return\"").unwrap();
    let basis = FaceContributionBasis {
        checked_plot_id: "checked/owner".into(),
        plan_id: "plan/body".into(),
        active_play_id: ActivePlayId::from("play/owner"),
        required_interaction_context: None,
    };
    let Reply::PatchbayFragment {
        revision,
        mut fragment,
        ..
    } = owner
        .patchbay_fragment(0, body.clone(), basis.clone(), true)
        .unwrap()
    else {
        panic!("fragment")
    };
    fragment.subjects.push(FaceSubject {
        identity: alloc::format!("body/{}", body.as_str()),
        role: FaceRole::Body,
        name: "Owner".into(),
    });
    for graph in &graphs {
        fragment.subjects.push(FaceSubject {
            identity: alloc::format!("plot/{}", graph.checked_plot_id.as_str()),
            role: FaceRole::Plot,
            name: graph.plot_name.clone(),
        });
    }
    let face = Face::new_with_semantics(
        u64::from(revision),
        FaceBasis {
            body_id: Some(body.clone()),
            wake_id: None,
            source_document_id: Some(SourceDocumentId::from("source/owner")),
            checked_plot_id: Some(basis.checked_plot_id.clone()),
            expanded_plot_id: Some(ExpandedPlotId::from("expanded/owner")),
            plan_id: Some(basis.plan_id.clone()),
            active_play_id: Some(basis.active_play_id.clone()),
            sign_ids: vec![],
        },
        fragment.subjects,
        fragment.relationships,
        fragment.properties,
        fragment.text,
        fragment.actions,
        fragment.disclosures,
    )
    .unwrap();
    let show = mask_fixture::available_mask_show(&face);
    assert!(matches!(
        owner
            .patchbay_publish(0, body, basis, face.clone(), show.clone())
            .unwrap(),
        Reply::PatchbayPublished { revision: 1, .. }
    ));
    Encounter {
        owner,
        face,
        show,
        graphs,
    }
}
fn input(
    e: &Encounter,
    action_id: &str,
    arguments: Vec<FaceInteractionArgument>,
) -> FaceInteraction {
    let action = e
        .face
        .actions
        .iter()
        .find(|action| action.identity == action_id)
        .unwrap();
    FaceInteraction::new(
        &e.face,
        &e.show,
        &action.identity,
        &action.target,
        arguments,
        1,
    )
    .unwrap()
}
fn selection(e: &Encounter) -> FaceInteraction {
    input(e, "patchbay.plot.1", vec![])
}
fn assert_quarantined(e: &mut Encounter, input: &[u8]) {
    assert!(matches!(
        e.owner.patchbay_consume(0, input),
        Err(Refusal::Malformed)
    ));
    assert_eq!(
        e.owner.selected[0], 1,
        "Root must not commit the returned target"
    );
    assert!(!e.owner.is_live());
    assert!(e.owner.graph(0).is_none());
    assert!(e.owner.graphs.iter().flatten().all(Option::is_none));
    assert!(e.owner.publications.iter().all(Option::is_none));
    assert!(e.owner.snapshots.iter().all(Option::is_none));
    assert!(e.owner.topology.is_none());
    assert_eq!(counts(), (1, 1));
    assert!(matches!(
        e.owner.patchbay_consume(0, input),
        Err(Refusal::Order)
    ));
    assert_eq!(counts(), (1, 1), "no replay or second service entry");
}
#[test]
fn genuine_sparse_selection_commits_once_after_root_return_admission() {
    let mut e = encounter();
    let input = selection(&e).encode();
    assert!(matches!(
        e.owner.patchbay_consume(0, &input).unwrap(),
        Reply::PatchbayConsumed {
            target: 3,
            revision: 2,
            proposal: None,
            refusal: None,
            ..
        }
    ));
    assert_eq!(e.owner.selected[0], 3);
    assert_eq!(e.owner.graph(0), Some(&e.graphs[1]));
    assert!(e.owner.is_live());
    assert!(e.owner.publications[0].is_none());
    assert_eq!(counts(), (1, 0));
    assert!(matches!(
        e.owner.patchbay_consume(0, &input),
        Err(Refusal::Order)
    ));
    assert_eq!(counts(), (1, 0));
}
#[test]
fn substituted_selection_targets_revisions_and_proposals_quarantine_before_commit() {
    for case in 0..11 {
        let mut e = encounter();
        let input = selection(&e).encode();
        substitute(move |reply| {
            let Reply::PatchbayConsumed {
                mut slot,
                mut target,
                mut revision,
                mut proposal,
                mut refusal,
            } = reply
            else {
                panic!("actual actor consume")
            };
            match case {
                0 => target = 1,
                1 => target = 2,
                2 => target = 255,
                3 => slot = 1,
                4 => revision = 1,
                5 => revision = 3,
                6 => revision = 0,
                7 => revision = u32::MAX,
                8 => proposal = Some(Proposal::OpenPatchbay),
                9 => refusal = Some(CanonicalRefusal::UnknownAction),
                10 => return Reply::Prepared,
                _ => unreachable!(),
            }
            Reply::PatchbayConsumed {
                slot,
                target,
                revision,
                proposal,
                refusal,
            }
        });
        assert_quarantined(&mut e, &input);
    }
}
#[test]
fn forged_success_cannot_override_stale_unknown_or_wrong_target_input() {
    for case in 0..4 {
        let mut e = encounter();
        let mut input = selection(&e);
        match case {
            0 => input.show_id = "show/stale".into(),
            1 => input.action_id = "patchbay.plot.99".into(),
            2 => input.target = alloc::format!("plot/{}", e.graphs[0].checked_plot_id.as_str()),
            3 => input.face_revision += 1,
            _ => unreachable!(),
        }
        substitute(|_| Reply::PatchbayConsumed {
            slot: 0,
            target: 3,
            revision: 2,
            proposal: None,
            refusal: None,
        });
        assert_quarantined(&mut e, &input.encode());
    }
}
#[test]
fn dishonest_refusal_cannot_move_root_and_next_projection_detects_guest_drift() {
    let mut e = encounter();
    let input = selection(&e).encode();
    substitute(|_| Reply::PatchbayConsumed {
        slot: 0,
        target: 1,
        revision: 1,
        proposal: None,
        refusal: Some(CanonicalRefusal::UnknownAction),
    });
    assert!(matches!(
        e.owner.patchbay_consume(0, &input).unwrap(),
        Reply::PatchbayConsumed {
            refusal: Some(_),
            ..
        }
    ));
    assert_eq!(e.owner.selected[0], 1);
    assert_eq!(e.owner.graph(0), Some(&e.graphs[0]));
    // The actual fixture actor did select3, then lied. Its next fragment cannot
    // become Root truth: canonical_identity fails and invalidates accessors.
    let body = e.face.basis.body_id.clone().unwrap();
    let basis = FaceContributionBasis {
        checked_plot_id: e.face.basis.checked_plot_id.clone().unwrap(),
        plan_id: e.face.basis.plan_id.clone().unwrap(),
        active_play_id: e.face.basis.active_play_id.clone().unwrap(),
        required_interaction_context: None,
    };
    assert!(matches!(
        e.owner.patchbay_fragment(0, body, basis, true),
        Err(Refusal::Malformed)
    ));
    assert!(!e.owner.is_live());
    assert!(e.owner.graph(0).is_none());
    assert_eq!(counts(), (1, 1));
}
fn edit_input(e: &Encounter) -> (String, FaceInteraction) {
    let subject = e.graphs[0].subject_identities().next().unwrap().to_owned();
    let action = e
        .face
        .actions
        .iter()
        .find(|action| action.identity == patchbay_application::EDIT_CURRENT_ACTION_ID)
        .unwrap();
    let interaction = input(
        e,
        &action.identity,
        vec![FaceInteractionArgument {
            name: "subject".into(),
            value_kind: action.arguments[0].contract.value_kind.as_str().into(),
            value: subject.as_bytes().to_vec(),
        }],
    );
    (subject, interaction)
}
#[test]
fn genuine_edit_proposal_retains_the_predecessor_graph_and_consumes_publication() {
    let mut e = encounter();
    let (subject, input) = edit_input(&e);
    let Reply::PatchbayConsumed {
        target: 1,
        revision: 2,
        proposal:
            Some(Proposal::EditCurrent(PatchbayApplicationRequest::EditCurrent {
                expanded_plot_id,
                subject_identity,
            })),
        refusal: None,
        ..
    } = e.owner.patchbay_consume(0, &input.encode()).unwrap()
    else {
        panic!("genuine admitted Edit proposal")
    };
    assert_eq!(expanded_plot_id, e.graphs[0].expanded_plot_id);
    assert_eq!(subject_identity, subject);
    assert_eq!(e.owner.selected[0], 1);
    assert_eq!(e.owner.graph(0), Some(&e.graphs[0]));
    assert!(e.owner.is_live());
    assert!(e.owner.publications[0].is_none());
    assert_eq!(counts(), (1, 0));
}
#[test]
fn substituted_edit_proposal_cannot_change_predecessor_graph_or_subject() {
    for case in 0..3 {
        let mut e = encounter();
        let (_, input) = edit_input(&e);
        let input = input.encode();
        let other_plot = e.graphs[1].expanded_plot_id.clone();
        substitute(move |reply| {
            let Reply::PatchbayConsumed {
                slot,
                mut target,
                revision,
                proposal:
                    Some(Proposal::EditCurrent(PatchbayApplicationRequest::EditCurrent {
                        mut expanded_plot_id,
                        mut subject_identity,
                    })),
                refusal: None,
            } = reply
            else {
                panic!("actual actor Edit proposal")
            };
            match case {
                0 => expanded_plot_id = other_plot,
                1 => subject_identity.push_str("/forged"),
                2 => target = 3,
                _ => unreachable!(),
            }
            Reply::PatchbayConsumed {
                slot,
                target,
                revision,
                proposal: Some(Proposal::EditCurrent(
                    PatchbayApplicationRequest::EditCurrent {
                        expanded_plot_id,
                        subject_identity,
                    },
                )),
                refusal: None,
            }
        });
        assert_quarantined(&mut e, &input);
    }
}

#[test]
fn root_read_only_custody_rejects_an_otherwise_genuine_edit_return() {
    let mut e = encounter();
    let (_, input) = edit_input(&e);
    // Deliberately give the actor a planned/editable fixture while Root retains
    // read-only custody. A valid Show and genuine actor proposal cannot grant it.
    e.owner.editable_targets[0][1] = false;
    assert!(matches!(
        e.owner.patchbay_consume(0, &input.encode()),
        Err(Refusal::Malformed)
    ));
    assert!(!e.owner.is_live());
    assert!(e.owner.graph(0).is_none());
    assert_eq!(counts(), (1, 1));
}

#[test]
fn root_read_only_custody_rejects_guest_advertised_edit_actions() {
    let mut e = encounter();
    let body = e.face.basis.body_id.clone().unwrap();
    let basis = FaceContributionBasis {
        checked_plot_id: "checked/owner".into(),
        plan_id: "plan/body".into(),
        active_play_id: ActivePlayId::from("play/owner"),
        required_interaction_context: None,
    };
    e.owner.editable_targets[0][1] = false;
    assert!(matches!(
        e.owner.patchbay_fragment(0, body, basis, true),
        Err(Refusal::Malformed)
    ));
    assert!(!e.owner.is_live());
    assert_eq!(counts(), (1, 1));
}
