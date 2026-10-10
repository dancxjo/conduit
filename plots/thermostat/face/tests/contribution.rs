use conduit_presentation::{PresentationActionAvailability, PresentationContributionBasis};
use conduit_thermostat_face::fragment;
use conduit_thermostat_plot::{Command, ThermostatState};
fn basis() -> PresentationContributionBasis {
    PresentationContributionBasis {
        checked_plot_id: "checked/thermostat".into(),
        plan_id: "plan/thermostat".into(),
        active_play_id: "play/thermostat".into(),
        required_interaction_context: None,
    }
}
#[test]
fn route_and_temperature_bounds_control_action_availability() {
    let state = ThermostatState::default();
    let readonly = fragment(&state, basis(), false).unwrap();
    assert!(readonly.actions.iter().all(|action| matches!(
        action.availability,
        PresentationActionAvailability::Unavailable { .. }
    )));
    for (target, limited, available) in [
        (100, "thermostat.lower", "thermostat.raise"),
        (300, "thermostat.raise", "thermostat.lower"),
    ] {
        let state = state.apply(Command::SetTarget(target)).unwrap();
        let face = fragment(&state, basis(), true).unwrap();
        assert!(matches!(
            face.actions
                .iter()
                .find(|a| a.identity == limited)
                .unwrap()
                .availability,
            PresentationActionAvailability::Unavailable { .. }
        ));
        assert_eq!(
            face.actions
                .iter()
                .find(|a| a.identity == available)
                .unwrap()
                .availability,
            PresentationActionAvailability::Available
        );
        assert!(face
            .text
            .iter()
            .any(|text| text.text == "Sensor unavailable"));
        assert_eq!(face.actions.len(), 11);
        face.validate_bounds().unwrap();
    }
}
