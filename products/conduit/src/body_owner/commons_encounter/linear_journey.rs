//! C3 is checked-source/runtime and software Mask proof, not installed Commons
//! discovery, native walkability, or an enactment of an owner action.
use super::*;
use conduit_face::*;
#[path = "linear_journey/contract.rs"]
mod contract;
#[path = "linear_journey/runtime.rs"]
mod runtime;

fn assert_graph(face: &Face, graph: &conduit_core::StructuredInfoValue) {
    for slot in ["hub", "workshop", "creche"] {
        let place = field(graph, slot).unwrap();
        let identity = text(field(place, "identity").unwrap()).unwrap();
        let subject = face
            .subjects
            .iter()
            .find(|s| s.identity == identity)
            .unwrap();
        assert_eq!(subject.name, text(field(place, "name").unwrap()).unwrap());
        assert!(
            matches!(&subject.role, FaceRole::Semantic(k) if k.as_str() == text(field(place, "role").unwrap()).unwrap())
        );
    }
    let mut expected = Vec::new();
    for slot in ["containment", "reachability"] {
        let StructuredInfoValueShape::Collection(relations) = field(graph, slot).unwrap().shape()
        else {
            panic!("Source relation collection")
        };
        for relation in relations {
            expected.push(FaceRelationship {
                source: text(field(relation, "source").unwrap()).unwrap().into(),
                target: text(field(relation, "target").unwrap()).unwrap().into(),
                kind: FaceRelationshipKind::Semantic(kind_id(
                    text(field(relation, "meaning").unwrap()).unwrap(),
                )),
            });
        }
    }
    assert_eq!(expected.len(), 4);
    let actual: Vec<_> = face.relationships.iter().filter(|r| matches!(&r.kind, FaceRelationshipKind::Semantic(k) if k.as_str().starts_with("commons/"))).cloned().collect();
    assert_eq!(actual, expected);
}

#[test]
fn actual_commons_plan_play_linear_journey_preserves_cross_mask_truth() {
    let (graph, runtime) = runtime::graph();
    let owner = owner("boot/commons-linear-journey");
    let host = owner.host.advertisement().clone();
    let before = owner.truth();
    let mut commons = CommonsEncounter::prepare().unwrap();
    assert_eq!(
        graph, commons.source.places,
        "Mask specimen must equal the actual Plan/Play output"
    );
    assert_eq!(
        runtime["source"],
        commons.source.checked.source_document_id.as_str()
    );
    let mut face = commons.project(Some(&owner), &host).unwrap();
    let mut selected = "commons/hub";
    assert_graph(&face, &graph);
    let mut stages = vec![contract::observe(&face, selected, &host)];
    let mut refusals = Vec::new();
    let mut portable_traversals = Vec::new();
    for (target, back) in [
        ("commons/workshop", false),
        ("commons/hub", true),
        ("commons/creche", false),
        ("commons/hub", true),
    ] {
        let (navigation, projection, mut state) = contract::navigation(&face, selected);
        if !back {
            let follow = navigation
                .follows
                .iter()
                .find(|f| f.source_subject == selected && f.target_subject == target)
                .unwrap();
            let reading =
                render_linear_navigation(&face, &navigation, &projection, state.cursor()).unwrap();
            assert!(reading
                .lines
                .iter()
                .any(|line| line.contains("AVAILABLE FOLLOW") && line.contains(&follow.identity)));
            state
                .navigate(
                    &face,
                    &navigation,
                    face.revision,
                    NavigationOperation::Follow(follow.identity.clone()),
                )
                .unwrap();
            assert_eq!(state.cursor().focus.as_deref(), Some(target));
            let reached =
                render_linear_navigation(&face, &navigation, &projection, state.cursor()).unwrap();
            assert!(reached
                .lines
                .iter()
                .any(|line| line.contains("CURSOR") && line.contains(target)));
            // Back is a bounded cursor return, not an authored reverse edge.
            state
                .navigate(&face, &navigation, face.revision, NavigationOperation::Back)
                .unwrap();
            assert_eq!(state.cursor().focus.as_deref(), Some(selected));
            let returned =
                render_linear_navigation(&face, &navigation, &projection, state.cursor()).unwrap();
            portable_traversals.push(serde_json::json!({
                "face":face.identity,"follow":follow,"listed":reading.lines,
                "reached":reached.lines,"returned":returned.lines,"history_limit":2,
            }));
            assert_eq!(state.history_len(), 0);
            assert_eq!(
                state
                    .navigate(&face, &navigation, face.revision, NavigationOperation::Back)
                    .unwrap_err(),
                NavigationRefusal::HistoryExhausted
            );
        } else {
            assert!(!navigation
                .follows
                .iter()
                .any(|f| f.source_subject == selected));
            let refused = commons
                .navigate(Some(&owner), &host, &face, target, false)
                .unwrap_err();
            assert!(refused.contains("source-refused"));
            refusals.push(refused);
        }
        let next = commons
            .navigate(Some(&owner), &host, &face, target, back)
            .unwrap();
        assert_eq!(commons.selected, target);
        assert!(
            render_linear_navigation(&next, &navigation, &projection, state.cursor()).is_err(),
            "previous Face navigation must not be rebound to a new context"
        );
        assert_graph(&next, &graph);
        assert_eq!(owner.truth(), before);
        face = next;
        selected = target;
        stages.push(contract::observe(&face, selected, &host));
    }
    assert_eq!(stages.len(), 5);
    assert_eq!(
        owner.truth(),
        before,
        "reading, inspection and context navigation cannot perform a Body action"
    );
    let evidence = serde_json::json!({
        "proof_class":"actual Commons checked Source/std Plan/Play; deterministic linear navigation; actual software terminal Mask effects; CPU graphical preparation",
        "portable_traversals":portable_traversals,"runtime":runtime,"owner_body":owner.session.evidence().body_id,"stages":stages,"wrong_direction_refusals":refusals,
        "owner_truth_unchanged":true,"native_qemu_walkability":false,"human_enactment":false,
    });
    let evidence = serde_json::to_string(&evidence).unwrap();
    assert!(evidence.len() <= 2 * 1024 * 1024);
    println!("COMMONS_LINEAR_JOURNEY={evidence}");
}

#[test]
fn commons_linear_navigation_refuses_unknown_reverse_stale_and_history_pressure() {
    let owner = owner("boot/commons-linear-refusals");
    let host = owner.host.advertisement().clone();
    let before = owner.truth();
    let mut commons = CommonsEncounter::prepare().unwrap();
    let hub = commons.project(Some(&owner), &host).unwrap();
    let (navigation, _, mut state) = contract::navigation(&hub, "commons/hub");
    let original = state.cursor().clone();
    for operation in [
        NavigationOperation::Follow("commons/forged".into()),
        NavigationOperation::Focus("commons/invented".into()),
    ] {
        assert!(state
            .navigate(&hub, &navigation, hub.revision, operation)
            .is_err());
        assert_eq!(state.cursor(), &original);
    }
    let follow = navigation
        .follows
        .iter()
        .find(|f| f.target_subject == "commons/workshop")
        .unwrap()
        .identity
        .clone();
    let mut full = NavigationState::new(&navigation, original.clone(), 0).unwrap();
    assert_eq!(
        full.navigate(
            &hub,
            &navigation,
            hub.revision,
            NavigationOperation::Follow(follow.clone())
        )
        .unwrap_err(),
        NavigationRefusal::HistoryFull
    );
    assert_eq!(full.cursor(), &original);
    assert_eq!(full.history_len(), 0);
    assert_eq!(
        state
            .navigate(
                &hub,
                &navigation,
                hub.revision + 1,
                NavigationOperation::Follow(follow.clone())
            )
            .unwrap_err(),
        NavigationRefusal::StaleFace
    );
    state
        .navigate(
            &hub,
            &navigation,
            hub.revision,
            NavigationOperation::Follow(follow.clone()),
        )
        .unwrap();
    let at_workshop = state.cursor().clone();
    assert_eq!(
        state
            .navigate(
                &hub,
                &navigation,
                hub.revision,
                NavigationOperation::Follow(follow)
            )
            .unwrap_err(),
        NavigationRefusal::UnknownRelationship
    );
    assert_eq!(state.cursor(), &at_workshop);
    let workshop = commons
        .navigate(Some(&owner), &host, &hub, "commons/workshop", false)
        .unwrap();
    for target in ["commons/creche", "commons/hub", "commons/invented"] {
        assert!(commons
            .navigate(Some(&owner), &host, &workshop, target, false)
            .is_err());
        assert_eq!(commons.project(Some(&owner), &host).unwrap(), workshop);
    }
    assert_eq!(owner.truth(), before);
}

#[test]
fn absent_owner_stays_explicit_without_inventing_linear_inspection_agency() {
    let host = StdHost::new().advertisement().clone();
    let mut commons = CommonsEncounter::prepare().unwrap();
    let hub = commons.project(None, &host).unwrap();
    let creche = commons
        .navigate(None, &host, &hub, "commons/creche", false)
        .unwrap();
    assert!(creche.actions.is_empty());
    let (navigation, projection, state) = contract::navigation(&creche, "commons/creche");
    let output = render_linear_navigation(&creche, &navigation, &projection, state.cursor())
        .unwrap()
        .lines
        .join("\n");
    assert!(output.contains("commons/inspection"));
    assert!(output.contains("context-unavailable"));
    assert!(!output.contains("ACTION "));
    assert!(creche.basis.body_id.is_none());
}
