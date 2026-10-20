//! Linear-derived proposals commit through BirthDraft with a checked Mask fixture.
//! This is Source conformance, not installed owner, Body Play or terminal I/O proof.
use super::*;

fn collection_id(linear: &Records, suffix: &str) -> String {
    let mut ids = linear
        .subjects
        .iter()
        .filter(|(id, (role, _))| role == "Collection" && id.ends_with(suffix));
    let id = ids.next().unwrap().0.clone();
    assert!(ids.next().is_none());
    id
}

fn proposal(
    face: &Face,
    show: &MaskShow,
    linear: &Records,
    action: &str,
    kind: &str,
    bytes: &[u8],
) -> Result<FaceInteraction, FaceInteractionRefusal> {
    let record = &linear.actions[action];
    // The declared specimen has one argument. Read its actual emitted name;
    // do not infer the binding from the option's label or ordinal.
    let tail = record["arguments"]
        .strip_prefix("[FaceActionArgument { name: ")
        .unwrap();
    let name = serde_json::Deserializer::from_str(tail)
        .into_iter::<String>()
        .next()
        .unwrap()
        .unwrap();
    assert!(name.len() <= 128);
    FaceInteraction::new(
        face,
        show,
        action,
        &records::string(&record["target"]),
        vec![FaceInteractionArgument {
            name,
            value_kind: kind.into(),
            value: bytes.to_vec(),
        }],
        1,
    )
}

#[test]
fn linear_exclusive_value_commits_once_and_replayed_or_forged_choices_have_no_effect() {
    let mut owner = draft();
    let face = owner.face(&basis()).unwrap();
    let linear = account(&face);
    let collection = collection_id(&linear, "/name-system/input-details");
    let option = linear
        .items(&collection)
        .into_iter()
        .find(|id| !linear.flag(id, "selected"))
        .unwrap();
    let bytes = linear.typed(&option, "choice-value").1;
    let action = linear.identity(&collection, "selection-action");
    assert!(records::membership(&linear.actions[&action]["arguments"]).contains(&bytes));
    let show = fixtures::acknowledged_fixture_show(&face);
    let before = owner.revision();
    let input = proposal(&face, &show, &linear, &action, UTF8_TEXT_VALUE_KIND, &bytes).unwrap();
    assert_eq!(owner.revision(), before);
    assert_eq!(
        owner.apply_face_interaction(&basis(), &show, &input),
        Ok(BirthActionOutcome::Changed)
    );
    assert_eq!(owner.requested_system().as_bytes(), bytes);
    assert_eq!(owner.revision(), before + 1);
    let committed = owner.face(&basis()).unwrap();
    assert!(account(&committed).flag(&option, "selected"));
    assert_eq!(
        owner.apply_face_interaction(&basis(), &show, &input),
        Err(BirthFaceRefusal::Interaction(
            FaceInteractionRefusal::StaleFace
        ))
    );
    let current = account(&committed);
    let show = fixtures::acknowledged_fixture_show(&committed);
    assert_eq!(
        proposal(
            &committed,
            &show,
            &current,
            &action,
            UTF8_TEXT_VALUE_KIND,
            b"Surprise me"
        )
        .err(),
        Some(FaceInteractionRefusal::ViolatedConstraint)
    );
    assert_eq!(owner.face(&basis()).unwrap(), committed);
}

#[test]
fn linear_boolean_roundtrip_refreshes_review_and_disabled_choice_cannot_commit() {
    let mut owner = draft();
    let face = owner.face(&basis()).unwrap();
    let linear = account(&face);
    let inventory = collection_id(&linear, "/initial-plots");
    let items = linear.items(&inventory);
    let selected = items.iter().find(|id| linear.flag(id, "selected")).unwrap();
    let action = linear.identity(selected, "boolean-change-action");
    let show = fixtures::acknowledged_fixture_show(&face);
    let before = owner.selection(owner.revision()).unwrap().workset;
    let input = proposal(&face, &show, &linear, &action, "value/bool", &[0]).unwrap();
    assert_eq!(owner.selection(owner.revision()).unwrap().workset, before);
    assert_eq!(
        owner.apply_face_interaction(&basis(), &show, &input),
        Ok(BirthActionOutcome::Changed)
    );
    let current = owner.face(&basis()).unwrap();
    let linear = account(&current);
    assert!(!linear.flag(selected, "selected"));
    assert_eq!(
        linear.count(&collection_id(&linear, "/reviewed-plots"), "item-count"),
        0
    );
    assert_eq!(
        owner.apply_face_interaction(&basis(), &show, &input),
        Err(BirthFaceRefusal::Interaction(
            FaceInteractionRefusal::StaleFace
        ))
    );
    let show = fixtures::acknowledged_fixture_show(&current);
    let disabled = items
        .iter()
        .map(|id| linear.identity(id, "boolean-change-action"))
        .find(|id| linear.actions[id]["availability"] != "available")
        .unwrap();
    assert_eq!(
        proposal(&current, &show, &linear, &disabled, "value/bool", &[1]).err(),
        Some(FaceInteractionRefusal::UnavailableAction)
    );
    assert_eq!(owner.face(&basis()).unwrap(), current);
    let input = proposal(&current, &show, &linear, &action, "value/bool", &[1]).unwrap();
    assert_eq!(
        owner.apply_face_interaction(&basis(), &show, &input),
        Ok(BirthActionOutcome::Changed)
    );
    assert_eq!(owner.selection(owner.revision()).unwrap().workset, before);
    let restored = account(&owner.face(&basis()).unwrap());
    assert!(restored.flag(selected, "selected"));
    assert_eq!(
        restored.count(&collection_id(&restored, "/reviewed-plots"), "item-count"),
        1
    );
}
