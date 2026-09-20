//! One bounded renderer-neutral semantic surface for a current Body.

use alloc::{format, string::String, vec, vec::Vec};
use conduit_body::{Body, BodyState, Wake, WakePlanState};
use conduit_core::{ActivePlayId, CheckedFormId, PlanId};
use serde::{Deserialize, Serialize};

use crate::{
    ApplicationEventKind, ApplicationView, ApplicationViewRefusal, Presentation, PresentationBasis,
    PresentationDisclosure, PresentationDisclosureLevel, PresentationError, PresentationProperty,
    PresentationPropertyValue, PresentationRelationship, PresentationRelationshipKind,
    PresentationRole, PresentationSubject, PresentationText,
};

mod action_resolution;
mod core_projection;
mod projection;
use core_projection::{append_execution_truth, append_operator_actions};
use projection::append_contribution;

pub const MAX_FACE_CONTRIBUTIONS: usize = 5;
pub const MAX_FACE_TRANSIENTS: usize = 2;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum FaceContext {
    Overview,
    Library,
    ResidentForm(CheckedFormId),
    Tutorial(CheckedFormId),
    Inspection(CheckedFormId),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum FaceContributionRole {
    Foreground,
    Tutorial,
    Inspection,
    Transient,
}

impl FaceContributionRole {
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
pub struct FaceContribution {
    pub role: FaceContributionRole,
    pub checked_form_id: CheckedFormId,
    pub plan_id: PlanId,
    pub active_play_id: ActivePlayId,
    pub view: ApplicationView,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum FaceFocus {
    Body,
    Contribution {
        role: FaceContributionRole,
        node_key: Option<String>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Face {
    pub context: FaceContext,
    pub focus: FaceFocus,
    pub presentation: Presentation,
    pub application_actions: Vec<FaceApplicationAction>,
    pub operator_actions: Vec<FaceOperatorAction>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FaceApplicationAction {
    pub surface_action_id: String,
    pub role: FaceContributionRole,
    pub checked_form_id: CheckedFormId,
    pub plan_id: PlanId,
    pub active_play_id: ActivePlayId,
    pub application_view_revision: u32,
    pub application_action_id: String,
    pub event: ApplicationEventKind,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FaceOperatorAction {
    pub surface_action_id: String,
    pub kind: FaceOperatorActionKind,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FaceOperatorActionKind {
    Wake,
    Lull,
    OpenOverview,
    OpenLibrary,
    OpenResidentForm(CheckedFormId),
    OpenInspection(CheckedFormId),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FaceRefusal {
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

impl Face {
    /// Projects Body truth first, then admits optional resident application
    /// contributions only from exact Plays in the current Wake.
    pub fn project(
        body: &Body,
        wake: Option<&Wake>,
        revision: u64,
        context: FaceContext,
        focus: FaceFocus,
        contributions: Vec<FaceContribution>,
    ) -> Result<Self, FaceRefusal> {
        body.validate().map_err(|_| FaceRefusal::InvalidBody)?;
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
        let mut actions = Vec::new();
        let mut inputs = Vec::new();
        let mut application_actions = Vec::new();
        let mut operator_actions = Vec::new();
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

        append_execution_truth(
            wake,
            &body_subject,
            &mut subjects,
            &mut relationships,
            &mut properties,
            &mut disclosures,
        );

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

        append_operator_actions(
            body,
            &contributions,
            &body_subject,
            &mut actions,
            &mut operator_actions,
        );

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
        .map_err(FaceRefusal::InvalidPresentation)?;
        Ok(Self {
            context,
            focus,
            presentation,
            application_actions,
            operator_actions,
        })
    }
}

fn validate_wake(body: &Body, wake: Option<&Wake>) -> Result<(), FaceRefusal> {
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

fn validate_contributions(
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
        contribution
            .view
            .validate()
            .map_err(FaceRefusal::InvalidApplicationView)?;
        if !body
            .workset
            .forms()
            .iter()
            .any(|form| form.checked_form_id == contribution.checked_form_id)
        {
            return Err(FaceRefusal::FormNotResident);
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
    if let Some(form) = context_form(context) {
        if !body
            .workset
            .forms()
            .iter()
            .any(|resident| &resident.checked_form_id == form)
        {
            return Err(FaceRefusal::InvalidContext);
        }
    }
    if let FaceFocus::Contribution { role, node_key } = focus {
        let Some(contribution) = contributions.iter().find(|item| &item.role == role) else {
            return Err(FaceRefusal::InvalidFocus);
        };
        if node_key
            .as_ref()
            .is_some_and(|key| !contribution.view.nodes.iter().any(|node| &node.key == key))
        {
            return Err(FaceRefusal::InvalidFocus);
        }
    }
    Ok(())
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

fn context_label(context: &FaceContext) -> &'static str {
    match context {
        FaceContext::Overview => "overview",
        FaceContext::Library => "library",
        FaceContext::ResidentForm(_) => "resident-form",
        FaceContext::Tutorial(_) => "tutorial",
        FaceContext::Inspection(_) => "inspection",
    }
}

fn context_form(context: &FaceContext) -> Option<&CheckedFormId> {
    match context {
        FaceContext::ResidentForm(form)
        | FaceContext::Tutorial(form)
        | FaceContext::Inspection(form) => Some(form),
        FaceContext::Overview | FaceContext::Library => None,
    }
}

fn focus_label(focus: &FaceFocus) -> String {
    match focus {
        FaceFocus::Body => "body".into(),
        FaceFocus::Contribution { role, node_key } => node_key.as_ref().map_or_else(
            || format!("contribution/{}", role.token()),
            |key| format!("contribution/{}/node/{key}", role.token()),
        ),
    }
}
