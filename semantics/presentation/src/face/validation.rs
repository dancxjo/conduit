//! Admission and collision laws for Face contributions.

use alloc::{format, vec};
use conduit_body::{Body, BodyState, Wake, WakePlanState, MAX_BODY_FRIENDLY_NAME_BYTES};

use super::{
    interaction_context_identity, FaceContext, FaceContribution, FaceFocus, FaceNames, FaceRefusal,
    MAX_FACE_CONTRIBUTIONS, MAX_FACE_TRANSIENTS,
};
use crate::FaceContributionRole;
use crate::PresentationFragmentError;
use crate::{presentation::MAX_PRESENTATION_TEXT_BYTES, FaceResidentPlotName};

pub(super) fn validate_names(body: &Body, names: FaceNames<'_>) -> Result<(), FaceRefusal> {
    if names
        .body_name
        .is_some_and(|name| name.trim().is_empty() || name.len() > MAX_BODY_FRIENDLY_NAME_BYTES)
    {
        return Err(FaceRefusal::InvalidBodyName);
    }
    for (index, entry) in names.resident_plots.iter().enumerate() {
        if entry.name.trim().is_empty() || entry.name.len() > MAX_PRESENTATION_TEXT_BYTES {
            return Err(FaceRefusal::InvalidPlotName);
        }
        if !body.workset.plots().iter().any(|resident| {
            resident.source_document_id == *entry.source_document_id
                && resident.checked_plot_id == *entry.checked_plot_id
        }) {
            return Err(FaceRefusal::PlotNameNotResident);
        }
        if names.resident_plots[index + 1..]
            .iter()
            .any(|other: &FaceResidentPlotName<'_>| {
                other.source_document_id == entry.source_document_id
                    && other.checked_plot_id == entry.checked_plot_id
            })
        {
            return Err(FaceRefusal::DuplicatePlotName);
        }
    }
    Ok(())
}

pub(super) fn validate_wake(body: &Body, wake: Option<&Wake>) -> Result<(), FaceRefusal> {
    match (&body.state, wake) {
        (BodyState::Lulled | BodyState::Fulfilled { .. }, None) => Ok(()),
        (BodyState::Lulled | BodyState::Fulfilled { .. }, Some(_)) => {
            Err(FaceRefusal::UnexpectedWake)
        }
        (BodyState::Awake { wake_id }, Some(wake)) => {
            wake.validate().map_err(|_| FaceRefusal::InvalidWake)?;
            if &wake.wake_id != wake_id
                || wake.body_id != body.body_id
                || wake.workset != body.workset
                || wake.workload_revision != body.workload_revision
            {
                return Err(FaceRefusal::InvalidWake);
            }
            Ok(())
        }
        (BodyState::Awake { .. }, None) => Err(FaceRefusal::MissingCurrentWake),
    }
}

pub(super) fn validate_contributions(
    body: &Body,
    wake: Option<&Wake>,
    context: &FaceContext,
    focus: &FaceFocus,
    contributions: &[FaceContribution],
) -> Result<(), FaceRefusal> {
    if contributions.len() > MAX_FACE_CONTRIBUTIONS {
        return Err(FaceRefusal::TooManyContributions);
    }
    if contributions
        .iter()
        .filter(|item| item.role == FaceContributionRole::Transient)
        .count()
        > MAX_FACE_TRANSIENTS
    {
        return Err(FaceRefusal::TooManyTransients);
    }
    for role in [
        FaceContributionRole::Foreground,
        FaceContributionRole::Tutorial,
        FaceContributionRole::Inspection,
    ] {
        if contributions
            .iter()
            .filter(|item| item.role == role)
            .count()
            > 1
        {
            return Err(FaceRefusal::DuplicateRole);
        }
    }
    for (index, contribution) in contributions.iter().enumerate() {
        validate_contribution(context, focus, contribution)?;
        if !body
            .workset
            .plots()
            .iter()
            .any(|plot| plot.checked_plot_id == contribution.checked_plot_id)
        {
            return Err(FaceRefusal::PlotNotResident);
        }
        if contributions[index + 1..]
            .iter()
            .any(|other| other.active_play_id == contribution.active_play_id)
        {
            return Err(FaceRefusal::DuplicatePlay);
        }
        let current = wake.is_some_and(|wake| {
            wake.plans.iter().any(|plan| {
                plan.plan_id == contribution.plan_id
                    && plan.state == WakePlanState::Playing
                    && plan.active_play_id.as_ref() == Some(&contribution.active_play_id)
            })
        });
        if !current {
            return Err(FaceRefusal::PlayNotCurrent);
        }
    }
    validate_contribution_identities(body, contributions)?;
    if let Some(plot) = context_plot(context) {
        if !body
            .workset
            .plots()
            .iter()
            .any(|resident| &resident.checked_plot_id == plot)
        {
            return Err(FaceRefusal::InvalidContext);
        }
    }
    validate_focus(focus, contributions)
}

fn validate_contribution(
    context: &FaceContext,
    focus: &FaceFocus,
    contribution: &FaceContribution,
) -> Result<(), FaceRefusal> {
    let fragment = &contribution.presentation;
    fragment
        .validate_bounds()
        .map_err(FaceRefusal::InvalidPresentationFragment)?;
    if fragment.basis.checked_plot_id != contribution.checked_plot_id
        || fragment.basis.plan_id != contribution.plan_id
        || fragment.basis.active_play_id != contribution.active_play_id
    {
        return Err(FaceRefusal::InvalidPresentationFragment(
            PresentationFragmentError::InvalidContextRequirement,
        ));
    }
    let context_identity = interaction_context_identity(context, focus);
    if fragment
        .basis
        .required_interaction_context
        .as_ref()
        .is_some_and(|required| required != &context_identity)
    {
        return Err(FaceRefusal::IncompatibleInteractionContext);
    }
    Ok(())
}

fn validate_focus(
    focus: &FaceFocus,
    contributions: &[FaceContribution],
) -> Result<(), FaceRefusal> {
    let FaceFocus::Contribution { role, node_key } = focus else {
        return Ok(());
    };
    let Some(contribution) = contributions.iter().find(|item| &item.role == role) else {
        return Err(FaceRefusal::InvalidFocus);
    };
    if let Some(key) = node_key {
        let found = contribution
            .presentation
            .subjects
            .iter()
            .any(|subject| &subject.identity == key);
        if !found {
            return Err(FaceRefusal::InvalidFocus);
        }
    }
    Ok(())
}

fn validate_contribution_identities(
    body: &Body,
    contributions: &[FaceContribution],
) -> Result<(), FaceRefusal> {
    let mut face_owned = vec![format!("body/{}", body.body_id.as_str())];
    face_owned.extend(
        body.workset
            .plots()
            .iter()
            .map(|plot| format!("plot/{}", plot.checked_plot_id.as_str())),
    );
    for contribution in contributions {
        let fragment = &contribution.presentation;
        if let Some(identity) = fragment
            .subjects
            .iter()
            .map(|subject| &subject.identity)
            .find(|identity| face_owned.contains(identity))
        {
            return Err(FaceRefusal::FaceOwnedIdentity(identity.clone()));
        }
    }
    for (index, contribution) in contributions.iter().enumerate() {
        let fragment = &contribution.presentation;
        for other in &contributions[index + 1..] {
            let other_fragment = &other.presentation;
            let collision = fragment
                .subjects
                .iter()
                .map(|subject| &subject.identity)
                .chain(fragment.actions.iter().map(|action| &action.identity))
                .find(|identity| {
                    other_fragment
                        .subjects
                        .iter()
                        .map(|subject| &subject.identity)
                        .chain(other_fragment.actions.iter().map(|action| &action.identity))
                        .any(|other_identity| other_identity == *identity)
                });
            if let Some(identity) = collision {
                return Err(FaceRefusal::ContributionIdentityCollision {
                    identity: identity.clone(),
                    first: contribution.checked_plot_id.clone(),
                    second: other.checked_plot_id.clone(),
                });
            }
        }
    }
    Ok(())
}

fn context_plot(context: &FaceContext) -> Option<&conduit_core::CheckedPlotId> {
    match context {
        FaceContext::ResidentPlot(plot)
        | FaceContext::Tutorial(plot)
        | FaceContext::Inspection(plot) => Some(plot),
        FaceContext::Overview | FaceContext::Library => None,
    }
}
