use conduit_birth_plot::{BirthDraft, BirthDraftRefusal, BirthPlotChoice};
use conduit_body::ResidentPlot;

fn draft() -> BirthDraft {
    BirthDraft::new(
        "550e8400-e29b-41d4-a716-446655440000".into(),
        vec![
            BirthPlotChoice {
                title: "Scratch".into(),
                search_text: "input/keyboard text/upper".into(),
                plot: ResidentPlot::new("source/scratch".into(), "checked/scratch".into()),
                refusal: None,
                selected: true,
            },
            BirthPlotChoice {
                title: "Chime".into(),
                search_text: "sound/chime audio/play".into(),
                plot: ResidentPlot::new("source/chime".into(), "checked/chime".into()),
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
        chosen.workset.plots(),
        &[ResidentPlot::new(
            "source/scratch".into(),
            "checked/scratch".into()
        )]
    );
}

#[test]
fn stale_input_empty_names_and_unavailable_plots_refuse_without_changing_selection() {
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
        Err(BirthDraftRefusal::UnavailablePlot)
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

#[test]
fn search_is_bounded_revisioned_state_that_preserves_selection() {
    let mut draft = draft();
    let selected = draft.selection(draft.revision()).unwrap().workset;
    for query in ["AUDIO", "no match", ""] {
        let revision = draft.revision();
        draft.search_plots(revision, query).unwrap();
        assert_eq!(draft.search(), query);
        assert_eq!(draft.revision(), revision + 1);
        assert_eq!(draft.selection(draft.revision()).unwrap().workset, selected);
    }
    let revision = draft.revision();
    for invalid in ["bad\nsearch".into(), "é".repeat(65)] {
        assert_eq!(
            draft.search_plots(revision, &invalid),
            Err(BirthDraftRefusal::InvalidSearch)
        );
        assert_eq!(draft.revision(), revision);
        assert_eq!(draft.search(), "");
    }
    let limit = "é".repeat(64);
    draft.search_plots(revision, &limit).unwrap();
    assert_eq!(draft.search(), limit);
    assert_eq!(
        draft.search_plots(revision, "stale"),
        Err(BirthDraftRefusal::StalePresentation)
    );
    assert_eq!(draft.search(), limit);
}

#[test]
fn full_inventory_and_zero_one_many_selections_keep_exact_workset_order() {
    let choices = (0..conduit_body::MAX_BODY_PLOTS)
        .map(|index| BirthPlotChoice {
            title: format!("Plot {index}"),
            search_text: String::new(),
            refusal: None,
            selected: false,
            plot: ResidentPlot::new(
                format!("source/{index}").into(),
                format!("checked/{index}").into(),
            ),
        })
        .collect();
    let mut draft =
        BirthDraft::new("11111111-2222-4333-8444-555555555555".into(), choices).unwrap();
    assert_eq!(
        draft.selection(draft.revision()),
        Err(BirthDraftRefusal::EmptySelection)
    );
    let revision = draft.revision();
    draft = draft.allow_idle_body();
    assert_eq!(draft.revision(), revision);
    assert!(draft.selection(revision).unwrap().workset.is_empty());
    draft.select(draft.revision(), 3, true).unwrap();
    assert_eq!(draft.selection(draft.revision()).unwrap().workset.len(), 1);
    draft.select(draft.revision(), 0, true).unwrap();
    assert_eq!(
        draft.selection(draft.revision()).unwrap().workset.plots(),
        &[
            draft.choices()[0].plot.clone(),
            draft.choices()[3].plot.clone()
        ]
    );
}

#[test]
fn inventory_bounds_and_exact_plot_identities_are_admitted_before_editing() {
    let choices = draft().choices().to_vec();
    let make = |choices| BirthDraft::new("550e8400-e29b-41d4-a716-446655440000".into(), choices);
    let refusal = |choices| make(choices).err();
    assert_eq!(refusal(vec![]), Some(BirthDraftRefusal::InvalidInventory));
    assert_eq!(
        refusal(vec![choices[0].clone(); conduit_body::MAX_BODY_PLOTS + 1]),
        Some(BirthDraftRefusal::InvalidInventory)
    );
    assert_eq!(
        refusal(vec![choices[0].clone(), choices[0].clone()]),
        Some(BirthDraftRefusal::InvalidInventory)
    );
    for (title, search, unavailable, selected) in [
        (String::new(), String::new(), None, false),
        ("é".repeat(33), String::new(), None, false),
        ("Plot".into(), "x".repeat(2_049), None, false),
        ("Plot".into(), String::new(), Some("x".repeat(257)), false),
        ("Plot".into(), String::new(), Some("No offer".into()), true),
    ] {
        let mut choice = choices[0].clone();
        choice.title = title;
        choice.search_text = search;
        choice.refusal = unavailable;
        choice.selected = selected;
        assert_eq!(
            refusal(vec![choice]),
            Some(BirthDraftRefusal::InvalidInventory)
        );
    }
    let mut choice = choices[0].clone();
    choice.title = "é".repeat(32);
    choice.search_text = "x".repeat(2_048);
    choice.refusal = Some("x".repeat(256));
    choice.selected = false;
    assert!(make(vec![choice]).is_ok());
}

#[test]
fn name_and_choice_edits_preserve_exact_revisions_and_refusal_precedence() {
    let mut draft = draft();
    let original = draft.selection(draft.revision()).unwrap();
    assert_eq!(
        draft.selection(0),
        Err(BirthDraftRefusal::StalePresentation)
    );
    assert_eq!(
        draft.select(0, usize::MAX, true),
        Err(BirthDraftRefusal::StalePresentation)
    );
    assert_eq!(
        draft.select(draft.revision(), usize::MAX, false),
        Err(BirthDraftRefusal::UnknownChoice)
    );
    assert_eq!(
        draft.edit_name(draft.revision(), "bad\nname".into()),
        Err(BirthDraftRefusal::InvalidName)
    );
    assert_eq!(
        draft.suggest(draft.revision(), "unknown"),
        Err(BirthDraftRefusal::Naming(
            conduit_birth_plot::names::NamingRefusal::UnknownSystem
        ))
    );
    assert_eq!(draft.selection(draft.revision()).unwrap(), original);
    assert_eq!(draft.suggestion().variation, 0);
    draft.suggest(draft.revision(), "roman").unwrap();
    assert_eq!(draft.suggestion().variation, 1);
    assert_eq!(draft.requested_system(), "roman");
    assert_eq!(draft.revision(), original.revision + 1);
    let limit = "é".repeat(32);
    draft.edit_name(draft.revision(), limit.clone()).unwrap();
    assert_eq!(draft.friendly_name(), limit);
    draft
        .edit_name(draft.revision(), "  Roseau  ".into())
        .unwrap();
    assert_eq!(draft.friendly_name(), "  Roseau  ");
    assert_eq!(
        draft.selection(draft.revision()).unwrap().friendly_name,
        "Roseau"
    );
    let revision = draft.revision();
    draft.select(revision, 1, false).unwrap();
    assert_eq!(draft.revision(), revision + 1);
}
