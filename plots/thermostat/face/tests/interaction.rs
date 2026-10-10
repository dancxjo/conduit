use conduit_presentation::{
    FaceInteraction, MaskShow, Presentation, PresentationBasis, PresentationContributionBasis,
};
use conduit_thermostat_face::{
    fragment, thermostat_command_from_contributed_interaction, ThermostatFaceError,
};
use conduit_thermostat_plot::{Command, Mode, ThermostatState};
#[path = "../../../../semantics/presentation/tests/common/mod.rs"]
mod common;
fn basis() -> PresentationContributionBasis {
    PresentationContributionBasis {
        checked_plot_id: "checked/thermostat".into(),
        plan_id: "plan/thermostat".into(),
        active_play_id: "play/thermostat".into(),
        required_interaction_context: None,
    }
}
fn face(state: &ThermostatState) -> (Presentation, MaskShow) {
    let basis = basis();
    let fragment = fragment(state, basis.clone(), true).unwrap();
    let face = Presentation::new_with_semantics(
        u64::from(state.revision),
        PresentationBasis {
            body_id: None,
            wake_id: None,
            source_document_id: Some("source/thermostat".into()),
            checked_plot_id: Some(basis.checked_plot_id),
            expanded_plot_id: Some("expanded/thermostat".into()),
            plan_id: Some(basis.plan_id),
            active_play_id: Some(basis.active_play_id),
            sign_ids: vec![],
        },
        fragment.subjects,
        fragment.relationships,
        fragment.properties,
        fragment.text,
        fragment.actions,
        fragment.disclosures,
    )
    .unwrap();
    let show = common::available_mask_show(&face);
    (face, show)
}
#[test]
fn canonical_shown_action_returns_one_command_and_refuses_wrong_owner_basis() {
    let state = ThermostatState::default();
    let (face, show) = face(&state);
    let interaction = FaceInteraction::new(
        &face,
        &show,
        "thermostat.mode.heat",
        "thermostat.mode.heat",
        vec![],
        1,
    )
    .unwrap();
    assert_eq!(
        thermostat_command_from_contributed_interaction(
            &state,
            &face,
            &show,
            &interaction,
            basis()
        ),
        Ok(Command::SetMode(Mode::Heat))
    );
    let mut wrong = basis();
    wrong.active_play_id = "play/foreign".into();
    assert_eq!(
        thermostat_command_from_contributed_interaction(&state, &face, &show, &interaction, wrong),
        Err(ThermostatFaceError::InvalidActionContract)
    );
    assert_eq!(
        thermostat_command_from_contributed_interaction(
            &state.apply(Command::SetMode(Mode::Heat)).unwrap(),
            &face,
            &show,
            &interaction,
            basis()
        ),
        Err(ThermostatFaceError::StaleState)
    );
}
#[test]
fn face_cannot_redefine_an_offered_thermostat_action_contract() {
    let state = ThermostatState::default();
    let (original, _) = face(&state);
    let mut actions = original.actions;
    actions
        .iter_mut()
        .find(|action| action.identity == "thermostat.raise")
        .unwrap()
        .intent = "foreign/raise@1".into();
    let face = Presentation::new_with_semantics(
        original.revision,
        original.basis,
        original.subjects,
        original.relationships,
        original.properties,
        original.text,
        actions,
        original.disclosures,
    )
    .unwrap();
    let show = common::available_mask_show(&face);
    let interaction = FaceInteraction::new(
        &face,
        &show,
        "thermostat.raise",
        "thermostat/target",
        vec![],
        1,
    )
    .unwrap();
    assert_eq!(
        thermostat_command_from_contributed_interaction(
            &state,
            &face,
            &show,
            &interaction,
            basis()
        ),
        Err(ThermostatFaceError::InvalidActionContract)
    );
}
