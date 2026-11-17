//! Comparison-only CPU oracle; complete Face/Show validation remains external.
extern crate std;
use super::*;
use conduit_core::{ActivePlayId, CheckedPlotId, PlanId};
use conduit_face::{FaceBasis, FaceInteractionContext};

fn fixture(actions: bool) -> (Face, FaceFragment) {
    let plans = conduit_patchbay_workbench_conformance::patchbay_mask_plans().unwrap();
    let mut port = PatchbayApplicationPort::open(
        &plans.direct_expanded,
        plans.direct.plan_id,
        PlanId::from("body-plan/base-index"),
    )
    .unwrap();
    port.select_prepared_plot(0).unwrap();
    let body: BodyId = serde_json::from_str("\"body/base-index\"").unwrap();
    let basis = FaceContributionBasis {
        checked_plot_id: CheckedPlotId::from("checked/base-index"),
        plan_id: PlanId::from("plan/base-index"),
        active_play_id: ActivePlayId::from("play/base-index"),
        required_interaction_context: None,
    };
    let base = port
        .face_fragment_with_graph(basis.clone(), &body, actions, false)
        .unwrap();
    let full = port.face_fragment(basis, &body, actions).unwrap();
    let face = Face {
        identity: serde_json::from_str("\"face/base-index\"").unwrap(),
        revision: 1,
        basis: FaceBasis {
            body_id: Some(body),
            wake_id: None,
            source_document_id: None,
            checked_plot_id: None,
            expanded_plot_id: None,
            plan_id: None,
            active_play_id: None,
            sign_ids: Vec::new(),
        },
        interaction_context: FaceInteractionContext::general(),
        subjects: full.subjects,
        relationships: full.relationships,
        composition: full.composition,
        properties: full.properties,
        text: full.text,
        actions: full.actions,
        disclosures: full.disclosures,
        temporal_references: full.temporal_references,
        temporal_facts: full.temporal_facts,
    };
    (face, base)
}
fn fast(face: &Face, base: &FaceFragment) -> bool {
    contains_other(face, base) && contains_indexed(&ContentIndex::new(face).unwrap(), base)
}
#[test]
fn indexed_base_preserves_original_membership_under_permutation_and_mutation() {
    for admitted in [false, true] {
        let (face, base) = fixture(admitted);
        assert!(contains(&face, &base));
        assert!(fast(&face, &base));
        macro_rules! parity {
            ($field:ident) => {{
                let mut permuted = face.clone();
                permuted.$field.reverse();
                assert_eq!(fast(&permuted, &base), contains(&permuted, &base));
                for item in &base.$field {
                    let mut changed = face.clone();
                    changed.$field.retain(|candidate| candidate != item);
                    assert!(!contains(&changed, &base));
                    assert_eq!(fast(&changed, &base), contains(&changed, &base));
                }
            }};
        }
        parity!(subjects);
        parity!(relationships);
        parity!(composition);
        parity!(properties);
        parity!(text);
        parity!(actions);
        parity!(disclosures);
        parity!(temporal_references);
        parity!(temporal_facts);
        // Same identity/key is insufficient: full retained value must match.
        let mut changed = face.clone();
        let subject = changed
            .subjects
            .iter_mut()
            .find(|s| s.identity == base.subjects[0].identity)
            .unwrap();
        subject.name.push_str(" changed");
        assert_eq!(fast(&changed, &base), contains(&changed, &base));
        assert!(!fast(&changed, &base));
        if let Some(action) = base.actions.first() {
            let mut changed = face.clone();
            let mut extra = action.clone();
            extra.identity.push_str("/extra");
            changed.actions.push(extra);
            assert!(!contains(&changed, &base));
            assert!(!fast(&changed, &base));
        }
    }
}
#[test]
fn actual_base_index_reduces_complete_value_candidate_comparisons() {
    let (mut face, base) = fixture(true);
    // Reverse actual emitted facts so the linear oracle cannot win from prefix order.
    face.subjects.reverse();
    face.relationships.reverse();
    face.properties.reverse();
    let mut linear = 0usize;
    macro_rules! counted {
        ($field:ident) => {
            base.$field.iter().all(|item| {
                face.$field.iter().any(|candidate| {
                    linear += 1;
                    candidate == item
                })
            })
        };
    }
    assert!(counted!(subjects) && counted!(relationships) && counted!(properties));
    let index = ContentIndex::new(&face).unwrap();
    super::index::reset_comparisons();
    assert!(contains_indexed(&index, &base));
    let indexed = super::index::comparisons();
    std::eprintln!("actual base complete-value candidates: linear={linear} indexed={indexed}; excludes hashing/index construction and is not RTC proof");
    assert!(indexed < linear);
}
