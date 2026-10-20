//! Full Source/check identity survives actual linear proposals and owner effects.
use super::*;
use crate::BirthPlotChoice;
use conduit_body::{BodyWorkset, ResidentPlot};

fn choices() -> Vec<BirthPlotChoice> {
    ["first", "second"]
        .into_iter()
        .map(|name| BirthPlotChoice {
            title: "同じ 🌻".into(),
            search_text: name.into(),
            plot: ResidentPlot::new(format!("source/{name}").into(), "checked/same-graph".into()),
            refusal: None,
            selected: true,
        })
        .collect()
}
fn owner(choices: Vec<BirthPlotChoice>) -> BirthDraft {
    BirthDraft::new("550e8400-e29b-41d4-a716-446655440000".into(), choices).unwrap()
}
#[test]
fn full_pair_linear_selection_changes_only_intended_pair_and_preserves_review_and_order() {
    let choices = choices();
    assert_eq!(
        BodyWorkset::from_plots(choices.iter().map(|c| c.plot.clone()))
            .unwrap()
            .len(),
        2
    );
    let mut source = owner(choices.clone());
    let face = source.face(&basis()).unwrap();
    let linear = verify_owner(&source, &face);
    let inventory = collection(&face, "/initial-plots").identity.clone();
    let review = collection(&face, "/reviewed-plots").identity.clone();
    let ids = linear.items(&inventory);
    assert_ne!(ids[0], ids[1]);
    assert_eq!(linear.subjects[&ids[0]].1, linear.subjects[&ids[1]].1);
    let target = &ids[1];
    let action = linear.identity(target, "boolean-change-action");
    let show = fixtures::acknowledged_fixture_show(&face);
    let input = commits::proposal(&face, &show, &linear, &action, "value/bool", &[0]).unwrap();
    assert_eq!(input.target, *target);
    assert_eq!(
        source.apply_face_interaction(&basis(), &show, &input),
        Ok(BirthActionOutcome::Changed)
    );
    let selected = source.selection(source.revision()).unwrap();
    assert_eq!(selected.workset.plots(), &[choices[0].plot.clone()]);
    assert!(source.choices()[0].selected && !source.choices()[1].selected);
    let committed = source.face(&basis()).unwrap();
    let now = verify_owner(&source, &committed);
    assert_eq!(now.items(&review).len(), 1);
    assert_eq!(
        source.apply_face_interaction(&basis(), &show, &input),
        Err(BirthFaceRefusal::Interaction(
            FaceInteractionRefusal::StaleFace
        ))
    );
    let fresh = fixtures::acknowledged_fixture_show(&committed);
    assert_eq!(
        commits::proposal(&committed, &fresh, &now, &action, "value/bool", &[2]).err(),
        Some(FaceInteractionRefusal::MalformedEncoding)
    );
    let mut retargeted =
        commits::proposal(&committed, &fresh, &now, &action, "value/bool", &[1]).unwrap();
    retargeted.target = ids[0].clone();
    assert!(
        source
            .apply_face_interaction(&basis(), &fresh, &retargeted)
            .is_err()
    );
    assert_eq!(source.face(&basis()).unwrap(), committed);
    let mut reversed = choices.clone();
    reversed.reverse();
    let reordered = owner(reversed).face(&basis()).unwrap();
    let reversed_ids = account(&reordered).items(&inventory);
    assert_eq!(reversed_ids, ids.iter().rev().cloned().collect::<Vec<_>>());
    source.search_plots(source.revision(), "second").unwrap();
    let filtered = source.face(&basis()).unwrap();
    let filtered = account(&filtered);
    assert_eq!(filtered.items(&inventory), vec![target.clone()]);
    assert_eq!(filtered.items(&review), now.items(&review));
    assert_eq!(
        source.selection(source.revision()).unwrap().workset,
        selected.workset
    );
}
#[test]
fn long_source_document_identity_is_not_narrowed_by_face_property_limits() {
    let choice = BirthPlotChoice {
        title: "Long source".into(),
        search_text: "long".into(),
        plot: ResidentPlot::new("s".repeat(1_024).into(), "checked/short".into()),
        refusal: None,
        selected: true,
    };
    assert_eq!(
        BodyWorkset::from_plots([choice.plot.clone()])
            .unwrap()
            .len(),
        1
    );
    let source = owner(vec![choice]);
    let face = source.face(&basis()).unwrap();
    let linear = verify_owner(&source, &face);
    let inventory = collection(&face, "/initial-plots");
    assert_eq!(linear.items(&inventory.identity).len(), 1);
    assert!(
        face.subjects
            .iter()
            .all(|s| s.identity.len() <= conduit_face::MAX_FACE_ID_BYTES)
    );
}
