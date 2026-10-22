//! CPU owner/Show fixtures; no native runtime or real Mask acceptance claim.
use conduit_body::BodyId;
use conduit_core::{ActivePlayId, ExpandedPlotId, SignId, SourceDocumentId, bind_active_play};
use conduit_face::*;
use conduitos_application_core::{data::*, models::Models};
use patchbay_graph::PatchbayGraph;

#[path = "../../../../semantics/face/tests/common/mod.rs"]
mod mask_fixture;

fn graph(name: &str) -> PatchbayGraph {
    let syntax = conduit_plot::parse_syntax_document(&format!("plot {name} {{}}"));
    let checked =
        conduit_plot::check_syntax_document(&syntax, &conduit_plot::StartupCatalog::new()).unwrap();
    let expanded = conduit_plot::expand_canonical_plot_for_authoring(
        &checked,
        name,
        &conduit_plot::ProfileCatalog::new(),
    )
    .unwrap();
    PatchbayGraph::from_expanded(&expanded.expanded).unwrap()
}
fn setup() -> (Models, [PatchbayGraph; 2], BodyId, FaceContributionBasis) {
    let graphs = [graph("alpha"), graph("beta")];
    let mut targets = core::array::from_fn(|_| None);
    for (slot, graph) in [1, 3].into_iter().zip(graphs.iter()) {
        targets[slot] = Some(Target {
            graph: graph.clone(),
            plan: format!("plan/target-{slot}").into(),
            body: "plan/body".into(),
        });
    }
    let mut models = Models::new([
        Some(Initial::Patchbay {
            targets,
            selected: 1,
        }),
        None,
        None,
        None,
    ])
    .unwrap();
    models.canonical(0).unwrap();
    (
        models,
        graphs,
        serde_json::from_str("\"body/selection\"").unwrap(),
        FaceContributionBasis {
            checked_plot_id: "checked/owner".into(),
            plan_id: "plan/body".into(),
            active_play_id: ActivePlayId::from("play/owner"),
            required_interaction_context: None,
        },
    )
}
fn face(
    models: &Models,
    graphs: &[PatchbayGraph; 2],
    body: &BodyId,
    basis: &FaceContributionBasis,
) -> Face {
    let Reply::PatchbayFragment {
        revision,
        mut fragment,
        ..
    } = models
        .patchbay_fragment(0, body.clone(), basis.clone(), true)
        .unwrap()
    else {
        panic!("fragment")
    };
    fragment.subjects.push(FaceSubject {
        identity: format!("body/{}", body.as_str()),
        role: FaceRole::Body,
        name: "Owner".into(),
    });
    for graph in graphs {
        fragment.subjects.push(FaceSubject {
            identity: format!("plot/{}", graph.checked_plot_id.as_str()),
            role: FaceRole::Plot,
            name: graph.plot_name.clone(),
        });
    }
    Face::new_with_semantics(
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
    .unwrap()
}
fn show(face: &Face, sequence: u64) -> MaskShow {
    let template = mask_fixture::available_mask_show(face);
    let terminal = template.planned_mask.show_placement();
    let active = bind_active_play(
        &template.planned_mask.plan.plan_id,
        &terminal.host_id,
        &terminal.boot_id,
        sequence,
    );
    MaskShow::prepared(
        &template.planned_mask,
        face,
        active,
        face.subjects[0].identity.clone(),
        "display/selection".into(),
        SignId::from("show/prepared"),
    )
    .unwrap()
    .transition(
        ManifestationLifecycle::Available,
        SignId::from("show/available"),
    )
    .unwrap()
}
fn publish(
    models: &mut Models,
    body: &BodyId,
    basis: &FaceContributionBasis,
    face: &Face,
    show: &MaskShow,
) {
    assert!(matches!(
        models
            .patchbay_publish(0, body.clone(), basis.clone(), face.clone(), show.clone())
            .unwrap(),
        Reply::PatchbayPublished { .. }
    ));
}
fn selection(face: &Face, show: &MaskShow, index: usize) -> FaceInteraction {
    let id = format!("patchbay.plot.{index}");
    let action = face
        .actions
        .iter()
        .find(|action| action.identity == id)
        .unwrap();
    FaceInteraction::new(face, show, &id, &action.target, vec![], 1).unwrap()
}
#[test]
fn canonical_sparse_selection_switches_exact_graphs_and_keeps_one_revision_sequence() {
    let (mut models, graphs, body, basis) = setup();
    assert_eq!(models.graph(0).unwrap(), &graphs[0]);
    let first = face(&models, &graphs, &body, &basis);
    for (index, graph) in graphs.iter().enumerate() {
        let action = first
            .actions
            .iter()
            .find(|action| action.identity == format!("patchbay.plot.{index}"))
            .unwrap();
        assert_eq!(
            action.target,
            format!("plot/{}", graph.checked_plot_id.as_str())
        );
    }
    let first_show = show(&first, 1);
    publish(&mut models, &body, &basis, &first, &first_show);
    let input = selection(&first, &first_show, 1).encode();
    assert!(matches!(
        models.patchbay_consume(0, &input).unwrap(),
        Reply::PatchbayConsumed {
            target: 3,
            revision: 2,
            proposal: None,
            refusal: None,
            ..
        }
    ));
    assert_eq!(models.graph(0).unwrap(), &graphs[1]);
    assert!(matches!(
        models.patchbay_consume(0, &input).unwrap(),
        Reply::PatchbayRefused {
            target: 3,
            revision: 2,
            refusal: CanonicalRefusal::NoPublication,
            ..
        }
    ));
    let second = face(&models, &graphs, &body, &basis);
    let second_show = show(&second, 2);
    publish(&mut models, &body, &basis, &second, &second_show);
    assert!(matches!(
        models.patchbay_consume(0, &input).unwrap(),
        Reply::PatchbayConsumed {
            target: 3,
            revision: 2,
            proposal: None,
            refusal: Some(_),
            ..
        }
    ));
    assert_eq!(models.graph(0).unwrap(), &graphs[1]);
    let replacement = show(&second, 3);
    publish(&mut models, &body, &basis, &second, &replacement);
    let mut forged = selection(&second, &replacement, 0);
    forged.action_id = "patchbay.plot.7".into();
    assert!(matches!(
        models.patchbay_consume(0, &forged.encode()).unwrap(),
        Reply::PatchbayConsumed {
            target: 3,
            revision: 2,
            proposal: None,
            refusal: Some(_),
            ..
        }
    ));
    assert_eq!(models.graph(0).unwrap(), &graphs[1]);
    let replacement = show(&second, 4);
    publish(&mut models, &body, &basis, &second, &replacement);
    let input = selection(&second, &replacement, 0).encode();
    assert!(matches!(
        models.patchbay_consume(0, &input).unwrap(),
        Reply::PatchbayConsumed {
            target: 1,
            revision: 3,
            proposal: None,
            refusal: None,
            ..
        }
    ));
    assert_eq!(models.graph(0).unwrap(), &graphs[0]);
}
#[test]
fn projected_inventory_requires_prepared_capacity_and_exact_body_plan() {
    let alpha = graph("alpha");
    let mut port = patchbay_application::PatchbayApplicationPort::open_graph(
        alpha.clone(),
        "plan/alpha".into(),
        "plan/body".into(),
    )
    .unwrap();
    assert!(
        port.append_prepared_graph(graph("beta"), "plan/beta".into(), &"plan/body".into())
            .is_err()
    );
    let mut port = port.with_prepared_graph_capacity(2).unwrap();
    assert!(
        port.append_prepared_graph(graph("beta"), "plan/beta".into(), &"plan/other-body".into())
            .is_err()
    );
    assert_eq!(
        port.append_prepared_graph(graph("beta"), "plan/beta".into(), &"plan/body".into())
            .unwrap(),
        1
    );
    assert!(
        port.append_prepared_graph(graph("gamma"), "plan/gamma".into(), &"plan/body".into())
            .is_err()
    );
    port.select_prepared_plot(1).unwrap();
    assert_eq!(port.selected_graph().unwrap().plot_name, "beta");
    assert_eq!(port.selected_plan_id().unwrap().as_str(), "plan/beta");
    assert_eq!(
        port.graph(),
        &alpha,
        "legacy first-graph accessor keeps its contract"
    );
}

fn staged(
    state: &mut conduitos_application_core::state::State,
    command: &Command,
) -> Result<(), Refusal> {
    use conduitos_application_core::wire::{Request, Response};
    let input = encode_command(command)?;
    let mut transfer = vec![0; conduitos_application_core::memory::TRANSFER_BYTES];
    state.execute(
        Request::Begin {
            bytes: input.len() as u32,
        },
        &mut transfer,
    )?;
    for (index, chunk) in input.chunks(transfer.len()).enumerate() {
        transfer[..chunk.len()].copy_from_slice(chunk);
        state.execute(
            Request::Write {
                offset: (index * transfer.len()) as u32,
                bytes: chunk.len() as u32,
            },
            &mut transfer,
        )?;
    }
    if matches!(command, Command::PrepareTarget { .. }) {
        assert_eq!(
            state.execute(Request::Preflight, &mut transfer)?,
            Response::Ready
        );
    }
    let Response::Blob { bytes } = state.execute(Request::Commit, &mut transfer)? else {
        return Err(Refusal::Malformed);
    };
    for offset in (0..bytes as usize).step_by(transfer.len()) {
        state.execute(
            Request::Read {
                offset: offset as u32,
                maximum: transfer.len() as u32,
            },
            &mut transfer,
        )?;
    }
    Ok(())
}
#[test]
fn invalid_later_inventory_entry_never_commits_a_partially_prepared_actor() {
    use conduitos_application_core::{state::State, wire::Request};
    for wrong_body in [false, true] {
        let mut state = State::default();
        staged(
            &mut state,
            &Command::PrepareInit {
                manifest: InitManifest {
                    slots: [
                        Some(InitSlot::Patchbay {
                            targets: 0b1010,
                            selected: 1,
                        }),
                        None,
                        None,
                        None,
                    ],
                },
            },
        )
        .unwrap();
        let first = Target {
            graph: graph("alpha"),
            plan: "plan/alpha".into(),
            body: "plan/body".into(),
        };
        staged(
            &mut state,
            &Command::PrepareTarget {
                slot: 0,
                target: 1,
                value: first.clone(),
            },
        )
        .unwrap();
        staged(&mut state, &Command::FinishTarget { slot: 0, target: 1 }).unwrap();
        let later = if wrong_body {
            Target {
                graph: graph("beta"),
                plan: "plan/beta".into(),
                body: "plan/other-body".into(),
            }
        } else {
            first
        };
        staged(
            &mut state,
            &Command::PrepareTarget {
                slot: 0,
                target: 3,
                value: later,
            },
        )
        .unwrap();
        assert_eq!(
            staged(&mut state, &Command::FinishTarget { slot: 0, target: 3 }),
            Err(Refusal::Application)
        );
        assert_eq!(
            state.execute(
                Request::Begin { bytes: 1 },
                &mut vec![0; conduitos_application_core::memory::TRANSFER_BYTES]
            ),
            Err(Refusal::Order),
            "a refused later entry must quarantine preparation rather than publish its earlier target"
        );
    }
}
