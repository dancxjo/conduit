//! Scoped collection facts and canonical reading traverse the same owner order.
use super::*;
use crate::BirthPlotChoice;
use conduit_body::ResidentPlot;
use conduit_face::prepare_face_collection;

fn precedence() -> FaceRelationshipKind {
    FaceRelationshipKind::Semantic(kind_id("document/precedes"))
}
fn check(face: &Face) {
    let plan = plan_face_utterances(face).unwrap();
    for suffix in [
        "/initial-plots",
        "/reviewed-plots",
        "/name-system/input-details",
    ] {
        let id = &collection(face, suffix).identity;
        let view = prepare_face_collection(face, id, 64).unwrap();
        let expected: Vec<_> = view
            .items()
            .iter()
            .map(|item| item.identity.clone())
            .collect();
        let observed: Vec<_> = plan
            .clauses
            .iter()
            .filter_map(|clause| {
                let FaceUtteranceProvenance::Subject(subject) = &clause.provenance else {
                    return None;
                };
                expected
                    .iter()
                    .find(|id| id.as_str() == subject.identity())
                    .cloned()
            })
            .collect();
        assert_eq!(
            observed, expected,
            "canonical aural traversal must retain checked collection order for {id}"
        );
        let edges: Vec<_> = face
            .relationships
            .iter()
            .filter(|r| {
                r.kind == precedence()
                    && (expected.contains(&r.source) || expected.contains(&r.target))
            })
            .collect();
        assert_eq!(edges.len(), expected.len().saturating_sub(1));
        for pair in expected.windows(2) {
            assert!(
                edges
                    .iter()
                    .any(|edge| edge.source == pair[0] && edge.target == pair[1])
            );
            assert!(
                matches!((property(face, &pair[0], "order"), property(face, &pair[1], "order")),
                (FacePropertyValue::Count(a), FacePropertyValue::Count(b)) if a < b)
            );
        }
    }
}
fn inventory(selected: &[usize]) -> Vec<BirthPlotChoice> {
    (0..6)
        .map(|n| BirthPlotChoice {
            title: "同じ 🌻".into(),
            search_text: if [0, 3, 5].contains(&n) {
                "visible".into()
            } else {
                "other".into()
            },
            plot: ResidentPlot::new(format!("source/{n}").into(), "checked/same-graph".into()),
            refusal: None,
            selected: selected.contains(&n),
        })
        .collect()
}
fn owner(choices: Vec<BirthPlotChoice>) -> BirthDraft {
    BirthDraft::new("550e8400-e29b-41d4-a716-446655440000".into(), choices).unwrap()
}
#[test]
fn canonical_reading_matches_all_three_profiles_despite_opaque_ids_and_storage_reorder() {
    let source = owner(inventory(&[0, 3, 5]));
    let face = source.face(&basis()).unwrap();
    let id = &collection(&face, "/initial-plots").identity;
    let view = prepare_face_collection(&face, id, 64).unwrap();
    let owner_order: Vec<_> = view.items().iter().map(|s| s.identity.clone()).collect();
    let mut lexical = owner_order.clone();
    lexical.sort();
    assert_ne!(
        lexical, owner_order,
        "fixture must expose accidental opaque-ID ordering"
    );
    check(&face);
    let mut shuffled = face;
    shuffled.subjects.reverse();
    shuffled.properties.reverse();
    // Relationship storage order is not traversal authority; check() below
    // compares edge sets independently of their physical vector order.
    shuffled.relationships.reverse();
    let shuffled = Face::new_with_semantics(
        shuffled.revision,
        shuffled.basis,
        shuffled.subjects,
        shuffled.relationships,
        shuffled.properties,
        shuffled.text,
        shuffled.actions,
        shuffled.disclosures,
    )
    .unwrap();
    check(&shuffled);
    let mut choices = inventory(&[0, 3, 5]);
    choices.reverse();
    check(&owner(choices).face(&basis()).unwrap());
}
#[test]
fn filtered_gaps_empty_and_singleton_profiles_have_only_actual_adjacent_member_edges() {
    for selected in [&[][..], &[3][..], &[0, 3, 5][..]] {
        let mut source = owner(inventory(selected));
        source.edit_name(source.revision(), "é".repeat(32)).unwrap();
        for query in ["visible", "no-result", ""] {
            source.search_plots(source.revision(), query).unwrap();
            check(&source.face(&basis()).unwrap());
        }
    }
    let source = owner(inventory(&[0]).into_iter().take(1).collect());
    check(&source.face(&basis()).unwrap());
}
