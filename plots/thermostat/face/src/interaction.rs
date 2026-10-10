//! Resolve only the owner's current contributed thermostat action contract.
use conduit_presentation::{
    FaceInteraction, MaskShow, Presentation, PresentationContributionBasis,
    PresentationPropertyValue,
};
use conduit_thermostat_plot::{Command, ThermostatState};
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ThermostatFaceError {
    InvalidState,
    StaleState,
    InvalidInteraction,
    InvalidActionContract,
    UnsupportedAction,
    CommandRefused,
}
pub fn thermostat_command_from_contributed_interaction(
    state: &ThermostatState,
    face: &Presentation,
    show: &MaskShow,
    interaction: &FaceInteraction,
    basis: PresentationContributionBasis,
) -> Result<Command, ThermostatFaceError> {
    state
        .validate()
        .map_err(|_| ThermostatFaceError::InvalidState)?;
    if !face.properties.iter().any(|property| {
        property.subject == "thermostat/device"
            && property.name == "thermostat-revision"
            && property.value == PresentationPropertyValue::Count(u64::from(state.revision))
    }) {
        return Err(ThermostatFaceError::StaleState);
    }
    interaction
        .validate_against(face, show)
        .map_err(|_| ThermostatFaceError::InvalidInteraction)?;
    let standalone = face.basis.checked_plot_id.as_ref() == Some(&basis.checked_plot_id)
        && face.basis.plan_id.as_ref() == Some(&basis.plan_id)
        && face.basis.active_play_id.as_ref() == Some(&basis.active_play_id);
    let contributed = face.properties.iter().any(|property| {
        property.subject.starts_with("contribution/foreground/")
            && property.name == "checked-plot-id"
            && property.value
                == PresentationPropertyValue::Identity(basis.checked_plot_id.as_str().into())
            && face.properties.iter().any(|other| {
                other.subject == property.subject
                    && other.name == "plan-id"
                    && other.value
                        == PresentationPropertyValue::Identity(basis.plan_id.as_str().into())
            })
            && face.properties.iter().any(|other| {
                other.subject == property.subject
                    && other.name == "active-play-id"
                    && other.value
                        == PresentationPropertyValue::Identity(basis.active_play_id.as_str().into())
            })
    });
    if !standalone && !contributed {
        return Err(ThermostatFaceError::InvalidActionContract);
    }
    let expected =
        super::fragment(state, basis, true).map_err(|_| ThermostatFaceError::InvalidState)?;
    let canonical = expected
        .actions
        .iter()
        .find(|action| action.identity == interaction.action_id)
        .ok_or(ThermostatFaceError::UnsupportedAction)?;
    if face
        .actions
        .iter()
        .find(|action| action.identity == interaction.action_id)
        != Some(canonical)
    {
        return Err(ThermostatFaceError::InvalidActionContract);
    }
    let command = super::command_for_action(state, &interaction.action_id)
        .map_err(|_| ThermostatFaceError::UnsupportedAction)?;
    state
        .apply(command)
        .map_err(|_| ThermostatFaceError::CommandRefused)?;
    Ok(command)
}
