//! Current native lifecycle action validation, preserving typed refusal categories.
use super::*;

pub(super) fn resolve_current_action(
    face: &Face,
    show: &MaskShow,
    interaction: &FaceInteraction,
) -> Result<patchbay_control::PatchbayAction, &'static str> {
    interaction
        .validate_against(face, show)
        .map_err(interaction_refusal)?;
    // This lifecycle adapter has no argument Fore. Validation must not turn a
    // future argument-bearing action into a request that discards its values.
    if !interaction.arguments.is_empty() {
        return Err("workspace-face-action-arguments-unsupported");
    }
    let action = face
        .resolve_action(face.revision, &interaction.action_id)
        .map_err(|_| "workspace-face-action-unavailable")?;
    if action.target != interaction.target {
        return Err("workspace-face-action-target-mismatch");
    }
    let name = action
        .identity
        .strip_prefix("action/")
        .and_then(|remaining| remaining.split_once('/'))
        .filter(|(_, target)| *target == action.target)
        .map(|(name, _)| name)
        .ok_or("workspace-face-action-identity-invalid")?;
    let semantic =
        patchbay_control::PatchbayAction::from_name(name).ok_or("workspace-face-action-unknown")?;
    if semantic.face_intent() != action.intent {
        return Err("workspace-face-action-intent-mismatch");
    }
    Ok(semantic)
}

fn interaction_refusal(reason: conduit_face::FaceInteractionRefusal) -> &'static str {
    use conduit_face::FaceInteractionRefusal::*;
    match reason {
        StaleFace => "workspace-face-stale-face",
        StaleShow => "workspace-face-stale-show",
        FailedShow => "workspace-face-failed-show",
        WrongTarget => "workspace-face-action-target-mismatch",
        UnknownAction => "workspace-face-action-unknown",
        UnavailableAction | RefusedAction => "workspace-face-action-unavailable",
        MissingArgument => "workspace-face-missing-argument",
        WrongValueKind => "workspace-face-wrong-value-kind",
        OversizeValue => "workspace-face-oversize-value",
        _ => "workspace-face-interaction-invalid",
    }
}
