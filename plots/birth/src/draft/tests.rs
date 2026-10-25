use super::*;
use alloc::{format, vec};

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
        Err(BirthDraftRefusal::StaleFace)
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

#[test]
fn presentation_readiness_matches_owned_selection_across_public_edits() {
    fn check(draft: &BirthDraft) {
        for revision in [0, draft.revision()] {
            assert_eq!(
                draft.selection_readiness(revision),
                draft.selection(revision).map(|_| ())
            );
        }
    }
    let choices = (0..MAX_BODY_PLOTS)
        .map(|index| BirthPlotChoice {
            title: format!("Écho {index}"),
            search_text: "音 audio".into(),
            plot: ResidentPlot::new(
                format!("source/{index}").into(),
                format!("checked/{index}").into(),
            ),
            refusal: None,
            selected: false,
        })
        .collect();
    let mut draft =
        BirthDraft::new("550e8400-e29b-41d4-a716-446655440000".into(), choices).unwrap();
    check(&draft);
    for mask in [1u16, u16::MAX, 0x5555, 0x8000, 0] {
        for index in 0..MAX_BODY_PLOTS {
            draft
                .select(draft.revision(), index, mask & (1 << index) != 0)
                .unwrap();
            check(&draft);
        }
        for name in ["", " \u{2003} ", "ΣΊΓΜΑ 音"] {
            draft.edit_name(draft.revision(), name.into()).unwrap();
            check(&draft);
        }
        draft.search_plots(draft.revision(), "absent 音").unwrap();
        check(&draft);
        draft.suggest(draft.revision(), "roman").unwrap();
        check(&draft);
    }
    draft = draft.allow_idle_body();
    check(&draft);
}
