use super::*;
use alloc::vec;

fn draft() -> BirthDraft {
    BirthDraft::new(
        "550e8400-e29b-41d4-a716-446655440000".into(),
        vec![BirthPlotChoice {
            title: "Scratch".into(),
            search_text: String::new(),
            plot: ResidentPlot::new("source/scratch".into(), "checked/scratch".into()),
            refusal: None,
            selected: true,
        }],
    )
    .unwrap()
}

#[test]
fn exhausted_revision_refuses_every_edit_without_changing_draft_state() {
    let mut draft = draft();
    draft.revision = u32::MAX;
    let selection = draft.selection(u32::MAX).unwrap();
    let suggestion = draft.suggestion().clone();
    let choices = draft.choices().to_vec();
    assert_eq!(
        draft.edit_name(u32::MAX, "new".into()),
        Err(BirthDraftRefusal::RevisionExhausted)
    );
    assert_eq!(
        draft.search_plots(u32::MAX, "search"),
        Err(BirthDraftRefusal::RevisionExhausted)
    );
    assert_eq!(
        draft.select(u32::MAX, 0, false),
        Err(BirthDraftRefusal::RevisionExhausted)
    );
    assert_eq!(
        draft.suggest(u32::MAX, "roman"),
        Err(BirthDraftRefusal::RevisionExhausted)
    );
    assert_eq!(
        draft.suggest(u32::MAX - 1, "roman"),
        Err(BirthDraftRefusal::StalePresentation)
    );
    assert_eq!(draft.revision(), u32::MAX);
    assert_eq!(draft.selection(u32::MAX).unwrap(), selection);
    assert_eq!(draft.suggestion(), &suggestion);
    assert_eq!(draft.choices(), choices);
    assert_eq!(draft.search(), "");
    assert_eq!(draft.requested_system(), "surprise");
}

#[test]
fn exhausted_suggestion_variation_refuses_before_mutation() {
    let mut draft = draft();
    draft.suggestion.variation = u32::MAX;
    let revision = draft.revision();
    let selection = draft.selection(revision).unwrap();
    let suggestion = draft.suggestion().clone();
    assert_eq!(
        draft.suggest(revision, "roman"),
        Err(BirthDraftRefusal::RevisionExhausted)
    );
    assert_eq!(draft.revision(), revision);
    assert_eq!(draft.selection(revision).unwrap(), selection);
    assert_eq!(draft.suggestion(), &suggestion);
    assert_eq!(draft.requested_system(), "surprise");
}
