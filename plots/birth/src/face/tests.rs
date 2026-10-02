use super::*;
use crate::{BirthPlotChoice, BirthPresentation};
use alloc::{format, string::String, vec};
use conduit_body::ResidentPlot;
use conduit_core::PlotIdentity;
use conduit_presentation::{
    ApplicationEventKind, FaceInteractionArgument, ManifestationLifecycle, UTF8_TEXT_VALUE_KIND,
    plan_face_utterances,
};
#[path = "fixtures.rs"]
mod fixtures;
fn basis() -> BirthFaceBasis {
    let plan = fixtures::producer_plan();
    BirthFaceBasis {
        host_id: "host/birth".into(),
        boot_id: "boot/birth".into(),
        encounter_id: "arrival/one".into(),
        producer_plot: PlotIdentity {
            source_document_id: plan.source_document_id.clone(),
            checked_plot_id: plan.checked_plot_id.clone(),
            expanded_plot_id: plan.expanded_plot_id.clone(),
        },
        producer_plan_id: plan.plan_id,
    }
}
fn draft() -> BirthDraft {
    BirthDraft::new(
        "550e8400-e29b-41d4-a716-446655440000".into(),
        vec![
            BirthPlotChoice {
                title: "Clock".into(),
                search_text: "time".into(),
                plot: ResidentPlot::new("source/clock".into(), "checked/clock".into()),
                refusal: None,
                selected: true,
            },
            BirthPlotChoice {
                title: "Speaker".into(),
                search_text: "sound".into(),
                plot: ResidentPlot::new("source/speaker".into(), "checked/speaker".into()),
                refusal: Some("This Host has no admitted audio output.".into()),
                selected: false,
            },
        ],
    )
    .unwrap()
}
fn input(
    draft: &BirthDraft,
    id: &str,
    value: Option<(&str, &[u8])>,
) -> Result<(MaskShow, FaceInteraction), FaceInteractionRefusal> {
    let face = draft.face(&basis()).unwrap();
    let show = fixtures::acknowledged_fixture_show(&face);
    let action = face
        .actions
        .iter()
        .find(|action| action.identity == id)
        .unwrap();
    let arguments = value
        .map(|(kind, bytes)| {
            vec![FaceInteractionArgument {
                name: "value".into(),
                value_kind: kind.into(),
                value: bytes.to_vec(),
            }]
        })
        .unwrap_or_default();
    let input = FaceInteraction::new(&face, &show, id, &action.target, arguments, 1)?;
    Ok((show, input))
}
fn apply(
    draft: &mut BirthDraft,
    id: &str,
    value: Option<(&str, &[u8])>,
) -> Result<BirthActionOutcome, BirthFaceRefusal> {
    let (show, input) = input(draft, id, value).map_err(BirthFaceRefusal::Interaction)?;
    draft.apply_face_interaction(&basis(), &show, &input)
}
#[test]
fn one_draft_and_action_sequence_match_widget_adapter_through_planned_mask_fixture() {
    let mut face = draft();
    let mut widget = draft();
    assert_eq!(
        apply(
            &mut face,
            "creche.name",
            Some((UTF8_TEXT_VALUE_KIND, "Lumière".as_bytes()))
        ),
        Ok(BirthActionOutcome::Changed)
    );
    widget
        .apply_event(
            widget.revision(),
            "creche.name",
            ApplicationEventKind::Input,
            "Lumière",
        )
        .unwrap();
    apply(&mut face, "creche.plot.0", Some(("value/bool", &[0]))).unwrap();
    widget
        .apply_event(
            widget.revision(),
            "creche.plot.0",
            ApplicationEventKind::Change,
            "false",
        )
        .unwrap();
    assert_eq!(
        apply(&mut face, "creche.birth", None),
        Err(BirthFaceRefusal::Interaction(
            FaceInteractionRefusal::UnavailableAction
        ))
    );
    apply(&mut face, "creche.plot.0", Some(("value/bool", &[1]))).unwrap();
    widget
        .apply_event(
            widget.revision(),
            "creche.plot.0",
            ApplicationEventKind::Change,
            "true",
        )
        .unwrap();
    assert_eq!(face.presentation(), widget.presentation());
    assert_eq!(
        apply(&mut face, "creche.birth", None).unwrap(),
        widget
            .apply_event(
                widget.revision(),
                "creche.birth",
                ApplicationEventKind::Activate,
                ""
            )
            .unwrap()
    );
    assert!(face.face(&basis()).unwrap().basis.body_id.is_none());
}
#[test]
fn exact_kind_value_availability_and_current_face_are_not_inferred() {
    let mut draft = draft();
    let before = draft.face(&basis()).unwrap();
    assert_eq!(
        input(&draft, "creche.plot.1", Some(("value/bool", &[1]))),
        Err(FaceInteractionRefusal::UnavailableAction)
    );
    let (show, mut forged) = input(&draft, "creche.plot.0", Some(("value/bool", &[1]))).unwrap();
    forged.arguments[0].value_kind = UTF8_TEXT_VALUE_KIND.into();
    assert_eq!(
        draft.apply_face_interaction(&basis(), &show, &forged),
        Err(BirthFaceRefusal::Interaction(
            FaceInteractionRefusal::WrongValueKind
        ))
    );
    let (show, mut forged) = input(&draft, "creche.plot.0", Some(("value/bool", &[1]))).unwrap();
    forged.arguments[0].value = vec![2];
    assert!(
        draft
            .apply_face_interaction(&basis(), &show, &forged)
            .is_err()
    );
    assert_eq!(draft.face(&basis()).unwrap(), before);
    let (show, edit) = input(&draft, "creche.name", Some((UTF8_TEXT_VALUE_KIND, b"Ada"))).unwrap();
    draft
        .apply_face_interaction(&basis(), &show, &edit)
        .unwrap();
    assert_eq!(
        draft.apply_face_interaction(&basis(), &show, &edit),
        Err(BirthFaceRefusal::Interaction(
            FaceInteractionRefusal::StaleFace
        ))
    );
    let (show, edit) = input(
        &draft,
        "creche.name",
        Some((UTF8_TEXT_VALUE_KIND, b"Grace")),
    )
    .unwrap();
    let mut other = basis();
    other.boot_id = "boot/other".into();
    assert_eq!(
        draft.apply_face_interaction(&other, &show, &edit),
        Err(BirthFaceRefusal::Interaction(
            FaceInteractionRefusal::StaleFace
        ))
    );
}
#[test]
fn closed_show_cannot_authorize_birth_and_arbitrary_confirmation_is_not_an_argument() {
    let mut draft = draft();
    let (show, birth) = input(&draft, "creche.birth", None).unwrap();
    let closed = show
        .transition(ManifestationLifecycle::Closed, "fixture/closed".into())
        .unwrap();
    assert_eq!(
        draft.apply_face_interaction(&basis(), &closed, &birth),
        Err(BirthFaceRefusal::Interaction(
            FaceInteractionRefusal::StaleShow
        ))
    );
    assert_eq!(
        input(&draft, "creche.birth", Some((UTF8_TEXT_VALUE_KIND, b"yes"))),
        Err(FaceInteractionRefusal::UnknownArgument)
    );
}
#[test]
fn editable_empty_name_search_and_declared_naming_choice_remain_existing_birth_actions() {
    let mut draft = draft();
    assert_eq!(
        apply(&mut draft, "creche.name", Some((UTF8_TEXT_VALUE_KIND, b""))),
        Ok(BirthActionOutcome::Changed)
    );
    assert!(readout(&draft).contains("Current value: (empty)."));
    assert!(readout(&draft).contains("InvalidName"));
    apply(
        &mut draft,
        "creche.search",
        Some((UTF8_TEXT_VALUE_KIND, b"sound")),
    )
    .unwrap();
    assert_eq!(draft.search(), "sound");
    assert!(
        !draft
            .face(&basis())
            .unwrap()
            .actions
            .iter()
            .any(|action| action.identity == "creche.plot.0")
    );
    assert_eq!(
        input(
            &draft,
            "creche.naming",
            Some((UTF8_TEXT_VALUE_KIND, b"not-a-tradition"))
        ),
        Err(FaceInteractionRefusal::ViolatedConstraint)
    );
    let (show, mut missing) = input(
        &draft,
        "creche.naming",
        Some((UTF8_TEXT_VALUE_KIND, b"surprise")),
    )
    .unwrap();
    missing.arguments.clear();
    assert_eq!(
        draft.apply_face_interaction(&basis(), &show, &missing),
        Err(BirthFaceRefusal::Interaction(
            FaceInteractionRefusal::MissingArgument
        ))
    );
    assert_eq!(
        apply(
            &mut draft,
            "creche.naming",
            Some((UTF8_TEXT_VALUE_KIND, b"surprise"))
        ),
        Ok(BirthActionOutcome::Changed)
    );
}
fn readout(draft: &BirthDraft) -> String {
    plan_face_utterances(&draft.face(&basis()).unwrap())
        .unwrap()
        .clauses
        .iter()
        .map(|clause| format!("{}\n", clause.text))
        .collect()
}
#[test]
fn deterministic_aural_projection_preserves_all_fields_choices_values_and_errors() {
    let draft = draft();
    let words = readout(&draft);
    assert_eq!(words, readout(&draft));
    for expected in [
        "A body of your own",
        "Friendly Body name",
        draft.friendly_name(),
        "Naming tradition",
        "Search Plots",
        "Clock",
        "Speaker",
        "Included",
        "Not included",
        "This Host has no admitted audio output.",
        "Birth Body",
    ] {
        assert!(words.contains(expected), "missing {expected}: {words}");
    }
    for (value, label) in draft.naming_systems() {
        assert!(words.contains(value));
        assert!(words.contains(label));
    }
    let face = draft.face(&basis()).unwrap();
    assert!(face.basis.body_id.is_none());
    assert!(face.basis.plan_id.is_some());
}

#[test]
fn producer_and_mask_substitution_refuse_while_zero_body_basis_remains_exact() {
    let mut draft = draft();
    let original = basis();
    let face = draft.face(&original).unwrap();
    let (show, birth) = input(&draft, "creche.birth", None).unwrap();
    assert!(show.validate(&face).is_ok());
    assert!(show.show.body_id.is_none());
    let mut replaced = original.clone();
    replaced.producer_plan_id = "plan/not-the-presented-producer".into();
    assert_eq!(
        draft.apply_face_interaction(&replaced, &show, &birth),
        Err(BirthFaceRefusal::Interaction(
            FaceInteractionRefusal::StaleFace
        ))
    );
    let mut wrong_mask = show.clone();
    wrong_mask.planned_mask.plan.plan_id = "plan/substituted-mask".into();
    assert!(
        draft
            .apply_face_interaction(&original, &wrong_mask, &birth)
            .is_err()
    );
    let prepared = fixtures::prepared_fixture_show(&face);
    assert_eq!(
        FaceInteraction::new(&face, &prepared, &birth.action_id, &birth.target, vec![], 1),
        Err(FaceInteractionRefusal::StaleShow)
    );
}
