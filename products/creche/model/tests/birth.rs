use conduit_birth_form::{BirthDraft, BirthDraftRefusal, BirthFormChoice};
use conduit_body::ResidentForm;
use conduit_creche_model::birth::{BirthActionOutcome, BirthActions, BirthPresentation};
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
fn current_birth_widget_projects_the_reviewed_friendly_name_and_birth_action() {
    let mut draft = draft();
    draft.edit_name(draft.revision(), "Roseau".into()).unwrap();
    let view = draft.presentation().unwrap().lower().unwrap();
    assert!(
        view.actions
            .iter()
            .any(|action| action.id == "creche.birth")
    );
    assert!(view.nodes.iter().any(|node| node.value == "Roseau"));
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
}

#[test]
fn full_inventory_and_every_tradition_fit_the_current_widget_view() {
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
    draft = draft.allow_idle_body();
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

#[test]
fn widget_protocol_bounds_and_stale_events_refuse_before_mutation() {
    let mut draft = draft();
    let revision = draft.revision();
    let selected = draft.selection(revision).unwrap();
    let oversized_action = "x".repeat(65);
    let oversized_value = "x".repeat(129);
    let oversized_name = "é".repeat(33);
    for (expected, action, event, value, refusal) in [
        (
            revision - 1,
            oversized_action.as_str(),
            ApplicationEventKind::Input,
            oversized_value.as_str(),
            BirthDraftRefusal::StalePresentation,
        ),
        (
            revision,
            oversized_action.as_str(),
            ApplicationEventKind::Input,
            "",
            BirthDraftRefusal::InvalidActionValue,
        ),
        (
            revision,
            "creche.name",
            ApplicationEventKind::Input,
            oversized_value.as_str(),
            BirthDraftRefusal::InvalidActionValue,
        ),
        (
            revision,
            "creche.name",
            ApplicationEventKind::Input,
            oversized_name.as_str(),
            BirthDraftRefusal::InvalidName,
        ),
        (
            revision,
            "creche.birth",
            ApplicationEventKind::Activate,
            "payload",
            BirthDraftRefusal::UnknownAction,
        ),
        (
            revision,
            "creche.suggest",
            ApplicationEventKind::Activate,
            "payload",
            BirthDraftRefusal::UnknownAction,
        ),
    ] {
        assert_eq!(
            draft.apply_event(expected, action, event, value),
            Err(refusal)
        );
        assert_eq!(draft.revision(), revision);
        assert_eq!(draft.selection(revision).unwrap(), selected);
    }
}

#[test]
fn widget_control_order_and_single_choice_search_gate_stay_adapter_owned() {
    use conduit_presentation::PresentationMechanism;

    let draft = draft();
    let view = draft.presentation().unwrap();
    let mut controls = Vec::new();
    // Native arrival consumes this order to route keyboard focus.
    for node in &view.root.children {
        match &node.mechanism {
            PresentationMechanism::FormField(field) => {
                controls.push(field.input_action.identity.as_str());
            }
            PresentationMechanism::Action(action) => controls.push(action.identity.as_str()),
            PresentationMechanism::ChoiceGroup { options, .. } => {
                controls.extend(
                    options
                        .iter()
                        .map(|choice| choice.change_action.identity.as_str()),
                );
            }
            _ => {}
        }
    }
    assert_eq!(
        controls,
        [
            "creche.name",
            "creche.naming",
            "creche.suggest",
            "creche.search",
            "creche.form.0",
            "creche.form.1",
            "creche.birth",
        ]
    );
    let mut single = BirthDraft::new(
        "550e8400-e29b-41d4-a716-446655440000".into(),
        vec![draft.choices()[0].clone()],
    )
    .unwrap();
    assert!(
        !single
            .presentation()
            .unwrap()
            .lower()
            .unwrap()
            .actions
            .iter()
            .any(|action| action.id == "creche.search")
    );
    let revision = single.revision();
    assert_eq!(
        single.apply_event(
            revision,
            "creche.search",
            ApplicationEventKind::Input,
            "form"
        ),
        Err(BirthDraftRefusal::UnknownAction)
    );
    assert_eq!(single.revision(), revision);
    assert_eq!(single.search(), "");
    // The renderer-neutral draft still accepts bounded search state directly.
    single.search_forms(revision, "form").unwrap();
    assert_eq!(single.search(), "form");
}
