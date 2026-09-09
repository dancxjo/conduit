use conduit_body::ResidentForm;
use conduit_creche_model::birth::BirthActionOutcome;
use conduit_creche_model::birth::{BirthDraft, BirthDraftRefusal, BirthFormChoice};
use conduit_presentation::ApplicationEventKind;

fn draft() -> BirthDraft {
    BirthDraft::new(
        "550e8400-e29b-41d4-a716-446655440000".into(),
        vec![
            BirthFormChoice {
                title: "Scratch".into(),
                search_text: "input/keyboard text/upper".into(),
                form: ResidentForm::new("source/scratch".into(), "checked/scratch".into()),
                refusal: None,
                selected: true,
            },
            BirthFormChoice {
                title: "Chime".into(),
                search_text: "sound/chime audio/play".into(),
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

#[test]
fn one_presented_action_boundary_serves_both_presenters_and_preserves_refusals() {
    let mut draft = draft();
    let view = draft.presentation().unwrap().lower().unwrap();
    assert!(
        view.actions
            .iter()
            .any(|action| action.id == "creche.naming")
    );
    assert!(
        view.nodes
            .iter()
            .any(|node| node.key == "name-system-control"
                && node.component == conduit_presentation::ApplicationComponent::Select)
    );
    assert_eq!(
        draft
            .apply_event(
                draft.revision(),
                "creche.name",
                ApplicationEventKind::Input,
                "Juniper"
            )
            .unwrap(),
        BirthActionOutcome::Changed
    );
    let selected = draft.selection(draft.revision()).unwrap();
    for (action, event, value, refusal) in [
        (
            "creche.name",
            ApplicationEventKind::Activate,
            "wrong event",
            BirthDraftRefusal::UnknownAction,
        ),
        (
            "creche.form.01",
            ApplicationEventKind::Change,
            "true",
            BirthDraftRefusal::UnknownAction,
        ),
        (
            "creche.form.0",
            ApplicationEventKind::Change,
            "yes",
            BirthDraftRefusal::InvalidActionValue,
        ),
        (
            "creche.form.1",
            ApplicationEventKind::Change,
            "true",
            BirthDraftRefusal::UnavailableForm,
        ),
        (
            "creche.naming",
            ApplicationEventKind::Change,
            "invented tradition",
            BirthDraftRefusal::UnknownChoice,
        ),
    ] {
        assert_eq!(
            draft.apply_event(draft.revision(), action, event, value),
            Err(refusal)
        );
        assert_eq!(draft.selection(draft.revision()).unwrap(), selected);
    }
    assert_eq!(
        draft
            .apply_event(
                draft.revision(),
                "creche.birth",
                ApplicationEventKind::Activate,
                ""
            )
            .unwrap(),
        BirthActionOutcome::Birth(selected)
    );
    let system: String = draft.naming_systems().nth(1).unwrap().0.into();
    draft
        .apply_event(
            draft.revision(),
            "creche.naming",
            ApplicationEventKind::Change,
            &system,
        )
        .unwrap();
    assert_eq!(
        draft.requested_system(),
        draft.naming_systems().nth(1).unwrap().0
    );
}

#[test]
fn search_preserves_selection_and_empty_search_results_remain_presentable() {
    let mut draft = draft();
    draft
        .apply_event(
            draft.revision(),
            "creche.search",
            ApplicationEventKind::Input,
            "AUDIO",
        )
        .unwrap();
    let view = draft.presentation().unwrap().lower().unwrap();
    assert!(view.nodes.iter().any(|node| node.text == "Chime"));
    assert!(
        !view
            .actions
            .iter()
            .any(|action| action.id == "creche.form.1")
    );
    assert!(
        !view
            .actions
            .iter()
            .any(|action| action.id == "creche.form.0")
    );
    assert_eq!(draft.selection(draft.revision()).unwrap().workset.len(), 1);
    draft
        .apply_event(
            draft.revision(),
            "creche.search",
            ApplicationEventKind::Input,
            "no match",
        )
        .unwrap();
    let view = draft.presentation().unwrap().lower().unwrap();
    assert!(
        view.nodes
            .iter()
            .any(|node| node.text.starts_with("No Forms match"))
    );
    assert_eq!(draft.selection(draft.revision()).unwrap().workset.len(), 1);
    let revision = draft.revision();
    assert_eq!(
        draft.search_forms(revision, "bad\nsearch"),
        Err(BirthDraftRefusal::InvalidSearch)
    );
    assert_eq!(draft.revision(), revision);
}

#[test]
fn full_inventory_and_every_tradition_fit_one_shared_view_and_idle_birth_is_explicit() {
    let choices = (0..conduit_body::MAX_BODY_FORMS)
        .map(|index| BirthFormChoice {
            title: format!("Form {index}"),
            search_text: String::new(),
            refusal: None,
            selected: false,
            form: ResidentForm::new(
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
    draft = draft.allow_idle_body();
    assert!(
        draft
            .selection(draft.revision())
            .unwrap()
            .workset
            .is_empty()
    );
    let view = draft.presentation().unwrap().lower().unwrap();
    assert_eq!(
        view.nodes
            .iter()
            .filter(|node| node.component == conduit_presentation::ApplicationComponent::Option)
            .count(),
        24
    );
    assert_eq!(
        view.actions
            .iter()
            .filter(|action| action.id.starts_with("creche.form."))
            .count(),
        16
    );
    view.encode().unwrap();
}
