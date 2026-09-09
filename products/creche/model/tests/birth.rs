use conduit_body::ResidentForm;
use conduit_creche_model::birth::{BirthDraft, BirthDraftRefusal, BirthFormChoice};

fn draft() -> BirthDraft {
    BirthDraft::new(
        "550e8400-e29b-41d4-a716-446655440000".into(),
        vec![
            BirthFormChoice {
                title: "Scratch".into(),
                form: ResidentForm::new("source/scratch".into(), "checked/scratch".into()),
                refusal: None,
                selected: true,
            },
            BirthFormChoice {
                title: "Chime".into(),
                form: ResidentForm::new("source/chime".into(), "checked/chime".into()),
                refusal: Some("No admitted audio output".into()),
                selected: false,
            },
        ],
    )
    .unwrap()
}

#[test]
fn naming_and_selection_produce_a_reviewable_workset_without_inventing_a_body() {
    let mut draft = draft();
    assert_eq!(draft.friendly_name(), "Gonçalo Pacheco Guerreiro");
    draft.edit_name(draft.revision(), "Roseau".into()).unwrap();
    let chosen = draft.selection(draft.revision()).unwrap();
    assert_eq!(chosen.friendly_name, "Roseau");
    assert_eq!(
        chosen.workset.forms(),
        &[ResidentForm::new(
            "source/scratch".into(),
            "checked/scratch".into()
        )]
    );
    let view = draft.presentation().unwrap().lower().unwrap();
    assert!(
        view.actions
            .iter()
            .any(|action| action.id == "creche.birth")
    );
    assert!(view.nodes.iter().any(|node| node.value == "Roseau"));
}

#[test]
fn stale_input_empty_names_and_unavailable_forms_refuse_without_changing_selection() {
    let mut draft = draft();
    let old_revision = draft.revision();
    draft.suggest(old_revision, "roman").unwrap();
    let selected = draft.selection(draft.revision()).unwrap();
    assert_eq!(
        draft.edit_name(old_revision, "Stale".into()),
        Err(BirthDraftRefusal::StalePresentation)
    );
    assert_eq!(
        draft.select(draft.revision(), 1, true),
        Err(BirthDraftRefusal::UnavailableForm)
    );
    assert_eq!(draft.selection(draft.revision()).unwrap(), selected);
    assert_eq!(
        draft.edit_name(draft.revision(), "é".repeat(33)),
        Err(BirthDraftRefusal::InvalidName)
    );
    draft.edit_name(draft.revision(), " ".into()).unwrap();
    assert_eq!(
        draft.selection(draft.revision()),
        Err(BirthDraftRefusal::InvalidName)
    );
    draft.edit_name(draft.revision(), "Roseau".into()).unwrap();
    draft.select(draft.revision(), 0, false).unwrap();
    assert_eq!(
        draft.selection(draft.revision()),
        Err(BirthDraftRefusal::EmptySelection)
    );
}
