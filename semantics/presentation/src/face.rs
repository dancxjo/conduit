//! One bounded renderer-neutral semantic surface for a current body.

use alloc::{boxed::Box, format, string::String, vec, vec::Vec};
use conduit_body::{Body, BodyState, Wake};
use conduit_core::{ActivePlayId, CheckedFormId, PlanId};
use serde::{Deserialize, Serialize};

use crate::{
    Presentation, PresentationBasis, PresentationContextBasis, PresentationDisclosure,
    PresentationDisclosureLevel, PresentationError, PresentationFragment,
    PresentationFragmentError, PresentationInteractionContext, PresentationProperty,
    PresentationPropertyValue, PresentationRelationship, PresentationRelationshipKind,
    PresentationRole, PresentationSubject, PresentationText,
};

mod action_resolution;
mod core_projection;
mod projection;
mod validation;
use core_projection::{append_execution_truth, append_operator_actions};
use projection::append_contribution;
use validation::{validate_contributions, validate_wake};

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
    pub presentation: Box<PresentationFragment>,
}

impl FaceContribution {
    pub fn from_presentation(role: FaceContributionRole, fragment: PresentationFragment) -> Self {
        Self {
            role,
            checked_form_id: fragment.basis.checked_form_id.clone(),
            plan_id: fragment.basis.plan_id.clone(),
            active_play_id: fragment.basis.active_play_id.clone(),
            presentation: Box::new(fragment),
        }
    }
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
    pub operator_actions: Vec<FaceOperatorAction>,
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
    InvalidPresentationFragment(PresentationFragmentError),
    IncompatibleInteractionContext,
    FaceOwnedIdentity(String),
    ContributionIdentityCollision {
        identity: String,
        first: CheckedFormId,
        second: CheckedFormId,
    },
    InvalidContext,
    InvalidFocus,
    InvalidPresentation(PresentationError),
    StaleAction,
    UnknownAction,
    UnavailableAction,
}

impl Face {
    /// Projects Body truth first, then admits optional resident application
    /// contributions only from exact plays in the current wake.
    pub fn project(
        body: &Body,
        wake: Option<&Wake>,
        revision: u64,
        context: FaceContext,
        focus: FaceFocus,
        mut contributions: Vec<FaceContribution>,
    ) -> Result<Self, FaceRefusal> {
        body.validate().map_err(|_| FaceRefusal::InvalidBody)?;
        validate_wake(body, wake)?;
        contributions.sort_by(|left, right| {
            (left.role.token(), left.active_play_id.as_str())
                .cmp(&(right.role.token(), right.active_play_id.as_str()))
        });
        validate_contributions(body, wake, &context, &focus, &contributions)?;

        let body_subject = format!("body/{}", body.body_id.as_str());
        let context_subject = format!("{body_subject}/context");
        let mut subjects = vec![
            PresentationSubject {
                identity: body_subject.clone(),
                role: PresentationRole::Body,
                name: "Current body".into(),
            },
            PresentationSubject {
                identity: context_subject.clone(),
                role: PresentationRole::Region,
                name: format!("Current {} context", context_label(&context)),
            },
        ];
        let mut relationships = vec![PresentationRelationship {
            source: body_subject.clone(),
            target: context_subject.clone(),
            kind: PresentationRelationshipKind::Contains,
        }];
        let mut composition = Vec::new();
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
        let mut temporal_references = Vec::new();
        let mut temporal_facts = Vec::new();

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
                name: format!("Resident Form {}", form.checked_form_id.as_str()),
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
                &mut composition,
                &mut properties,
                &mut text,
                &mut actions,
                &mut disclosures,
                &mut temporal_references,
                &mut temporal_facts,
            );
        }

        let mut sign_ids = body.sign_ids.clone();
        if let Some(wake) = wake {
            sign_ids.extend(wake.sign_ids.iter().cloned());
        }
        sign_ids.sort();
        sign_ids.dedup();
        let mut presentation = Presentation::new_with_semantics(
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
            disclosures,
        )
        .map_err(FaceRefusal::InvalidPresentation)?;
        presentation.temporal_references = temporal_references;
        presentation.temporal_facts = temporal_facts;
        presentation.composition = composition;
        let presentation = presentation
            .with_interaction_context(PresentationInteractionContext {
                identity: interaction_context_identity(&context, &focus),
                basis: vec![PresentationContextBasis {
                    source: body_subject,
                    relationship: PresentationRelationshipKind::Contains,
                    target: context_subject,
                }],
            })
            .map_err(FaceRefusal::InvalidPresentation)?;
        Ok(Self {
            context,
            focus,
            presentation,
            operator_actions,
        })
    }
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

fn focus_label(focus: &FaceFocus) -> String {
    match focus {
        FaceFocus::Body => "body".into(),
        FaceFocus::Contribution { role, node_key } => node_key.as_ref().map_or_else(
            || format!("contribution/{}", role.token()),
            |key| format!("contribution/{}/node/{key}", role.token()),
        ),
    }
}

pub(super) fn interaction_context_identity(context: &FaceContext, focus: &FaceFocus) -> String {
    let context = match context {
        FaceContext::Overview => "overview".into(),
        FaceContext::Library => "library".into(),
        FaceContext::ResidentForm(form) => format!("resident-form/{}", form.as_str()),
        FaceContext::Tutorial(form) => format!("tutorial/{}", form.as_str()),
        FaceContext::Inspection(form) => format!("inspection/{}", form.as_str()),
    };
    format!("face/context/{context}/focus/{}", focus_label(focus))
}
