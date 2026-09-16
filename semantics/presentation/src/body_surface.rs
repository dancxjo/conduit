//! One bounded renderer-neutral semantic surface for a current Body.

use alloc::{format, string::String, vec, vec::Vec};
use conduit_body::{Body, BodyState, Wake, WakePlanState};
use conduit_core::{ActivePlayId, CheckedFormId};

use crate::{
    ApplicationEventKind, ApplicationView, ApplicationViewRefusal, Presentation,
    PresentationAction, PresentationActionAvailability, PresentationActionRefusal,
    PresentationBasis, PresentationDisclosure, PresentationDisclosureLevel, PresentationError,
    PresentationProperty, PresentationPropertyValue, PresentationRelationship,
    PresentationRelationshipKind, PresentationRole, PresentationSubject, PresentationText,
};

mod projection;
use projection::append_contribution;

pub const MAX_BODY_SURFACE_CONTRIBUTIONS: usize = 5;
pub const MAX_BODY_SURFACE_TRANSIENTS: usize = 2;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BodySurfaceContext {
    Overview,
    Library,
    ResidentForm(CheckedFormId),
    Tutorial(CheckedFormId),
    Inspection(CheckedFormId),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BodySurfaceContributionRole {
    Foreground,
    Tutorial,
    Inspection,
    Transient,
}

impl BodySurfaceContributionRole {
    fn token(self) -> &'static str {
        match self {
            Self::Foreground => "foreground",
            Self::Tutorial => "tutorial",
            Self::Inspection => "inspection",
            Self::Transient => "transient",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BodySurfaceContribution {
    pub role: BodySurfaceContributionRole,
    pub checked_form_id: CheckedFormId,
    pub active_play_id: ActivePlayId,
    pub view: ApplicationView,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BodySurfaceFocus {
    Body,
    Contribution {
        role: BodySurfaceContributionRole,
        node_key: Option<String>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BodySurface {
    pub context: BodySurfaceContext,
    pub focus: BodySurfaceFocus,
    pub presentation: Presentation,
    pub application_actions: Vec<BodySurfaceApplicationAction>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BodySurfaceApplicationAction {
    pub surface_action_id: String,
    pub role: BodySurfaceContributionRole,
    pub checked_form_id: CheckedFormId,
    pub active_play_id: ActivePlayId,
    pub application_view_revision: u32,
    pub application_action_id: String,
    pub event: ApplicationEventKind,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BodySurfaceRefusal {
    InvalidBody,
    InvalidWake,
    MissingCurrentWake,
    UnexpectedWake,
    TooManyContributions,
    TooManyTransients,
    DuplicateRole,
    DuplicatePlay,
    FormNotResident,
    PlayNotCurrent,
    InvalidApplicationView(ApplicationViewRefusal),
    InvalidContext,
    InvalidFocus,
    InvalidPresentation(PresentationError),
    StaleAction,
    UnknownAction,
    UnavailableAction,
}

impl BodySurface {
    /// Projects Body truth first, then admits optional resident application
    /// contributions only from exact Plays in the current Wake.
    pub fn project(
        body: &Body,
        wake: Option<&Wake>,
        revision: u64,
        context: BodySurfaceContext,
        focus: BodySurfaceFocus,
        contributions: Vec<BodySurfaceContribution>,
    ) -> Result<Self, BodySurfaceRefusal> {
        body.validate()
            .map_err(|_| BodySurfaceRefusal::InvalidBody)?;
        validate_wake(body, wake)?;
        validate_contributions(body, wake, &context, &focus, &contributions)?;

        let body_subject = format!("body/{}", body.body_id.as_str());
        let context_subject = format!("{body_subject}/context");
        let mut subjects = vec![
            PresentationSubject {
                identity: body_subject.clone(),
                role: PresentationRole::Body,
                label: "Body".into(),
                accessibility_name: "Current Body".into(),
            },
            PresentationSubject {
                identity: context_subject.clone(),
                role: PresentationRole::Region,
                label: context_label(&context).into(),
                accessibility_name: format!("Current {} context", context_label(&context)),
            },
        ];
        let mut relationships = vec![PresentationRelationship {
            source: body_subject.clone(),
            target: context_subject.clone(),
            kind: PresentationRelationshipKind::Contains,
        }];
        let mut properties = vec![
            identity_property(&body_subject, "body-id", body.body_id.as_str()),
            PresentationProperty {
                subject: body_subject.clone(),
                name: "lifecycle-state".into(),
                value: PresentationPropertyValue::Text(lifecycle_label(&body.state).into()),
            },
            PresentationProperty {
                subject: body_subject.clone(),
                name: "workload-revision".into(),
                value: PresentationPropertyValue::Count(body.workload_revision),
            },
            PresentationProperty {
                subject: context_subject.clone(),
                name: "operator-context".into(),
                value: PresentationPropertyValue::Text(context_label(&context).into()),
            },
            PresentationProperty {
                subject: context_subject.clone(),
                name: "operator-focus".into(),
                value: PresentationPropertyValue::Text(focus_label(&focus)),
            },
        ];
        let mut text = vec![PresentationText {
            subject: body_subject.clone(),
            text: format!(
                "Body {} is {} with {} resident Form(s) at workload revision {}.",
                body.body_id.as_str(),
                lifecycle_label(&body.state),
                body.workset.len(),
                body.workload_revision
            ),
        }];
        let mut actions = lifecycle_action(body, &body_subject).into_iter().collect();
        let mut inputs = Vec::new();
        let mut application_actions = Vec::new();
        let mut disclosures = vec![
            PresentationDisclosure {
                subject: body_subject.clone(),
                level: PresentationDisclosureLevel::Primary,
            },
            PresentationDisclosure {
                subject: context_subject.clone(),
                level: PresentationDisclosureLevel::Context,
            },
        ];

        for form in body.workset.forms() {
            let form_subject = format!("form/{}", form.checked_form_id.as_str());
            subjects.push(PresentationSubject {
                identity: form_subject.clone(),
                role: PresentationRole::Form,
                label: form.checked_form_id.as_str().into(),
                accessibility_name: format!("Resident Form {}", form.checked_form_id.as_str()),
            });
            relationships.push(PresentationRelationship {
                source: body_subject.clone(),
                target: form_subject.clone(),
                kind: PresentationRelationshipKind::Contains,
            });
            properties.extend([
                identity_property(
                    &form_subject,
                    "source-document-id",
                    form.source_document_id.as_str(),
                ),
                identity_property(
                    &form_subject,
                    "checked-form-id",
                    form.checked_form_id.as_str(),
                ),
                PresentationProperty {
                    subject: form_subject.clone(),
                    name: "workload-membership".into(),
                    value: PresentationPropertyValue::Text("resident".into()),
                },
            ]);
            disclosures.push(PresentationDisclosure {
                subject: form_subject,
                level: PresentationDisclosureLevel::Context,
            });
        }

        for (index, contribution) in contributions.iter().enumerate() {
            append_contribution(
                index,
                contribution,
                &context_subject,
                &mut subjects,
                &mut relationships,
                &mut properties,
                &mut text,
                &mut actions,
                &mut inputs,
                &mut disclosures,
                &mut application_actions,
            );
        }

        let mut sign_ids = body.sign_ids.clone();
        if let Some(wake) = wake {
            sign_ids.extend(wake.sign_ids.iter().cloned());
        }
        sign_ids.sort();
        sign_ids.dedup();
        let presentation = Presentation::new_with_interactions(
            revision,
            PresentationBasis {
                body_id: Some(body.body_id.clone()),
                wake_id: wake.map(|value| value.wake_id.clone()),
                source_document_id: None,
                checked_form_id: None,
                expanded_form_id: None,
                plan_id: None,
                active_play_id: None,
                sign_ids,
            },
            subjects,
            relationships,
            properties,
            text,
            actions,
            inputs,
            disclosures,
        )
        .map_err(BodySurfaceRefusal::InvalidPresentation)?;
        Ok(Self {
            context,
            focus,
            presentation,
            application_actions,
        })
    }

    /// Resolves an exact current semantic action back to its admitted resident
    /// application without invoking renderer callbacks or Body authority.
    pub fn resolve_application_action(
        &self,
        presentation_revision: u64,
        action_id: &str,
    ) -> Result<&BodySurfaceApplicationAction, BodySurfaceRefusal> {
        self.presentation
            .resolve_action(presentation_revision, action_id)
            .map_err(|refusal| match refusal {
                PresentationActionRefusal::StaleRevision => BodySurfaceRefusal::StaleAction,
                PresentationActionRefusal::UnknownAction => BodySurfaceRefusal::UnknownAction,
                PresentationActionRefusal::Unavailable { .. }
                | PresentationActionRefusal::Refused { .. } => {
                    BodySurfaceRefusal::UnavailableAction
                }
            })?;
        self.application_actions
            .iter()
            .find(|action| action.surface_action_id == action_id)
            .ok_or(BodySurfaceRefusal::UnknownAction)
    }
}

fn validate_wake(body: &Body, wake: Option<&Wake>) -> Result<(), BodySurfaceRefusal> {
    match (&body.state, wake) {
        (BodyState::Lulled | BodyState::Fulfilled { .. }, None) => Ok(()),
        (BodyState::Lulled | BodyState::Fulfilled { .. }, Some(_)) => {
            Err(BodySurfaceRefusal::UnexpectedWake)
        }
        (BodyState::Awake { wake_id }, Some(wake)) => {
            wake.validate()
                .map_err(|_| BodySurfaceRefusal::InvalidWake)?;
            if &wake.wake_id != wake_id
                || wake.body_id != body.body_id
                || wake.workset != body.workset
                || wake.workload_revision != body.workload_revision
            {
                return Err(BodySurfaceRefusal::InvalidWake);
            }
            Ok(())
        }
        (BodyState::Awake { .. }, None) => Err(BodySurfaceRefusal::MissingCurrentWake),
    }
}

fn validate_contributions(
    body: &Body,
    wake: Option<&Wake>,
    context: &BodySurfaceContext,
    focus: &BodySurfaceFocus,
    contributions: &[BodySurfaceContribution],
) -> Result<(), BodySurfaceRefusal> {
    if contributions.len() > MAX_BODY_SURFACE_CONTRIBUTIONS {
        return Err(BodySurfaceRefusal::TooManyContributions);
    }
    if contributions
        .iter()
        .filter(|item| item.role == BodySurfaceContributionRole::Transient)
        .count()
        > MAX_BODY_SURFACE_TRANSIENTS
    {
        return Err(BodySurfaceRefusal::TooManyTransients);
    }
    for role in [
        BodySurfaceContributionRole::Foreground,
        BodySurfaceContributionRole::Tutorial,
        BodySurfaceContributionRole::Inspection,
    ] {
        if contributions
            .iter()
            .filter(|item| item.role == role)
            .count()
            > 1
        {
            return Err(BodySurfaceRefusal::DuplicateRole);
        }
    }
    for (index, contribution) in contributions.iter().enumerate() {
        contribution
            .view
            .validate()
            .map_err(BodySurfaceRefusal::InvalidApplicationView)?;
        if !body
            .workset
            .forms()
            .iter()
            .any(|form| form.checked_form_id == contribution.checked_form_id)
        {
            return Err(BodySurfaceRefusal::FormNotResident);
        }
        if contributions[index + 1..]
            .iter()
            .any(|other| other.active_play_id == contribution.active_play_id)
        {
            return Err(BodySurfaceRefusal::DuplicatePlay);
        }
        let current = wake.is_some_and(|wake| {
            wake.plans.iter().any(|plan| {
                plan.state == WakePlanState::Playing
                    && plan.active_play_id.as_ref() == Some(&contribution.active_play_id)
            })
        });
        if !current {
            return Err(BodySurfaceRefusal::PlayNotCurrent);
        }
    }
    if let Some(form) = context_form(context) {
        if !body
            .workset
            .forms()
            .iter()
            .any(|resident| &resident.checked_form_id == form)
        {
            return Err(BodySurfaceRefusal::InvalidContext);
        }
    }
    if let BodySurfaceFocus::Contribution { role, node_key } = focus {
        let Some(contribution) = contributions.iter().find(|item| &item.role == role) else {
            return Err(BodySurfaceRefusal::InvalidFocus);
        };
        if node_key
            .as_ref()
            .is_some_and(|key| !contribution.view.nodes.iter().any(|node| &node.key == key))
        {
            return Err(BodySurfaceRefusal::InvalidFocus);
        }
    }
    Ok(())
}

fn lifecycle_action(body: &Body, target: &str) -> Option<PresentationAction> {
    let (identity, intent, label) = match body.state {
        BodyState::Lulled => ("wake", "conduit.intent/wake@1", "Wake"),
        BodyState::Awake { .. } => ("lull", "conduit.intent/lull@1", "Lull"),
        BodyState::Fulfilled { .. } => return None,
    };
    Some(PresentationAction {
        identity: format!("body/action/{identity}/{}", body.workload_revision),
        intent: intent.into(),
        target: target.into(),
        label: label.into(),
        disclosure: PresentationDisclosureLevel::CurrentAction,
        availability: PresentationActionAvailability::Available,
    })
}

fn identity_property(subject: &str, name: &str, value: &str) -> PresentationProperty {
    PresentationProperty {
        subject: subject.into(),
        name: name.into(),
        value: PresentationPropertyValue::Identity(value.into()),
    }
}

fn lifecycle_label(state: &BodyState) -> &'static str {
    match state {
        BodyState::Lulled => "lulled",
        BodyState::Awake { .. } => "awake",
        BodyState::Fulfilled { .. } => "fulfilled",
    }
}

fn context_label(context: &BodySurfaceContext) -> &'static str {
    match context {
        BodySurfaceContext::Overview => "overview",
        BodySurfaceContext::Library => "library",
        BodySurfaceContext::ResidentForm(_) => "resident-form",
        BodySurfaceContext::Tutorial(_) => "tutorial",
        BodySurfaceContext::Inspection(_) => "inspection",
    }
}

fn context_form(context: &BodySurfaceContext) -> Option<&CheckedFormId> {
    match context {
        BodySurfaceContext::ResidentForm(form)
        | BodySurfaceContext::Tutorial(form)
        | BodySurfaceContext::Inspection(form) => Some(form),
        BodySurfaceContext::Overview | BodySurfaceContext::Library => None,
    }
}

fn focus_label(focus: &BodySurfaceFocus) -> String {
    match focus {
        BodySurfaceFocus::Body => "body".into(),
        BodySurfaceFocus::Contribution { role, node_key } => node_key.as_ref().map_or_else(
            || format!("contribution/{}", role.token()),
            |key| format!("contribution/{}/node/{key}", role.token()),
        ),
    }
}
