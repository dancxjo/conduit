//! Actual producer order across all three profiles; no installed Mask proof.
use super::collection_profiles::{collection, property};
use super::*;
use crate::BirthPlotChoice;
use alloc::vec::Vec;
use conduit_body::ResidentPlot;
use conduit_face::{FacePropertyValue, prepare_face_collection};

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
        let mut internal: Vec<_> = face
            .relationships
            .iter()
            .filter(|r| {
                r.kind == precedence()
                    && expected.contains(&r.source)
                    && expected.contains(&r.target)
            })
            .map(|r| (r.source.clone(), r.target.clone()))
            .collect();
        let mut adjacent: Vec<_> = expected
            .windows(2)
            .map(|pair| (pair[0].clone(), pair[1].clone()))
            .collect();
        assert_eq!(internal.len(), expected.len().saturating_sub(1));
        internal.sort();
        adjacent.sort();
        assert_eq!(internal, adjacent, "exact adjacent member chain for {id}");
        for pair in expected.windows(2) {
            assert!(
                matches!((property(face, &pair[0], "order"), property(face, &pair[1], "order")),
                (FacePropertyValue::Count(a), FacePropertyValue::Count(b)) if a < b)
            );
        }

        // Compact inventory identities need explicit choice-before-field
        // ordering. Preserve the old complete touching-edge check while
        // accounting for these accepted external reading anchors as well.
        let mut complete = adjacent;
        if suffix == "/initial-plots"
            && let Some(last) = expected.last()
        {
            let root = id.strip_suffix(suffix).unwrap();
            for field in ["name-system", "plot-search"] {
                let target = format!("{root}/{field}");
                if face
                    .subjects
                    .iter()
                    .any(|subject| subject.identity == target)
                {
                    complete.push((last.clone(), target));
                }
            }
        }
        let mut touching: Vec<_> = face
            .relationships
            .iter()
            .filter(|r| {
                r.kind == precedence()
                    && (expected.contains(&r.source) || expected.contains(&r.target))
            })
            .map(|r| (r.source.clone(), r.target.clone()))
            .collect();
        touching.sort();
        complete.sort();
        assert_eq!(
            touching, complete,
            "complete member/boundary edges for {id}"
        );
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

fn faces(source: &BirthDraft) -> [Face; 2] {
    [
        source.face(&basis()).unwrap(),
        source.host_owned_face(&host_owned_basis()).unwrap(),
    ]
}

fn reverse_storage(mut face: Face) -> Face {
    face.subjects.reverse();
    face.properties.reverse();
    face.relationships.reverse();
    Face::new_with_semantics_and_temporal(
        face.revision,
        face.basis,
        face.subjects,
        face.relationships,
        face.properties,
        face.text,
        face.actions,
        face.disclosures,
        face.temporal_references,
        face.temporal_facts,
    )
    .unwrap()
    .with_interaction_context(face.interaction_context)
    .unwrap()
    .with_composition(face.composition)
    .unwrap()
}

#[test]
fn canonical_reading_matches_all_three_profiles_despite_opaque_ids_and_storage_reorder() {
    let source = owner(inventory(&[0, 3, 5]));
    for face in faces(&source) {
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
        check(&reverse_storage(face));
    }
    let mut choices = inventory(&[0, 3, 5]);
    choices.reverse();
    for face in faces(&owner(choices)) {
        check(&face);
        check(&reverse_storage(face));
    }
}

#[test]
fn filtered_gaps_empty_and_singleton_profiles_have_only_actual_adjacent_member_edges() {
    for selected in [&[][..], &[3][..], &[0, 3, 5][..]] {
        let mut source = owner(inventory(selected));
        source.edit_name(source.revision(), "é".repeat(32)).unwrap();
        for query in ["visible", "no-result", ""] {
            source.search_plots(source.revision(), query).unwrap();
            for face in faces(&source) {
                check(&face);
                check(&reverse_storage(face));
            }
        }
    }
    let source = owner(inventory(&[0]).into_iter().take(1).collect());
    for face in faces(&source) {
        check(&face);
        check(&reverse_storage(face));
    }
}
