//! Adapt the living Patchbay projection into Conduit's portable Presentation value.

use conduit_body::{Body, BodyLifecycleError, BodyState, Wake, WakeLifecycle};
use conduit_core::SignId;
use conduit_presentation::{
    Presentation, PresentationBasis, PresentationError, PresentationProperty,
    PresentationPropertyValue, PresentationRelationship, PresentationRelationshipKind,
    PresentationRole, PresentationSubject, PresentationText,
};

use crate::{GraphItemKind, PartsView, PatchbayPresentation};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PortableProjectionError {
    InvalidBody(BodyLifecycleError),
    InvalidWake(BodyLifecycleError),
    LifecycleMismatch,
    MissingCheckedForm,
    PlanMismatch,
    PlayMismatch,
    InvalidPresentation(PresentationError),
}

impl core::fmt::Display for PortableProjectionError {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(
            formatter,
            "cannot project portable Patchbay presentation: {self:?}"
        )
    }
}

impl std::error::Error for PortableProjectionError {}

pub(super) struct ContentBuilder {
    pub(super) subjects: Vec<PresentationSubject>,
    pub(super) relationships: Vec<PresentationRelationship>,
    pub(super) properties: Vec<PresentationProperty>,
    pub(super) text: Vec<PresentationText>,
}

impl ContentBuilder {
    fn new() -> Self {
        Self {
            subjects: Vec::new(),
            relationships: Vec::new(),
            properties: Vec::new(),
            text: Vec::new(),
        }
    }

    pub(super) fn subject(
        &mut self,
        role: PresentationRole,
        label: impl Into<String>,
        accessibility_name: impl Into<String>,
    ) -> String {
        let identity = format!("patchbay/subject/{}", self.subjects.len());
        self.subject_with_identity(identity, role, label, accessibility_name)
    }

    pub(super) fn subject_with_identity(
        &mut self,
        identity: impl Into<String>,
        role: PresentationRole,
        label: impl Into<String>,
        accessibility_name: impl Into<String>,
    ) -> String {
        let identity = identity.into();
        if let Some(existing) = self
            .subjects
            .iter()
            .find(|subject| subject.identity == identity)
        {
            debug_assert_eq!(existing.role, role);
            return identity;
        }
        self.subjects.push(PresentationSubject {
            identity: identity.clone(),
            role,
            label: nonempty(label.into()),
            accessibility_name: nonempty(accessibility_name.into()),
        });
        identity
    }

    pub(super) fn contains(&mut self, source: &str, target: &str) {
        self.relationships.push(PresentationRelationship {
            source: source.into(),
            target: target.into(),
            kind: PresentationRelationshipKind::Contains,
        });
    }

    pub(super) fn describes(&mut self, source: &str, target: &str) {
        self.relationships.push(PresentationRelationship {
            source: source.into(),
            target: target.into(),
            kind: PresentationRelationshipKind::Describes,
        });
    }

    pub(super) fn line(&mut self, subject: &str, value: impl Into<String>) {
        self.text.push(PresentationText {
            subject: subject.into(),
            text: nonempty(value.into()),
        });
    }

    pub(super) fn property(&mut self, subject: &str, name: &str, value: PresentationPropertyValue) {
        if self.properties.iter().any(|property| {
            property.subject == subject && property.name == name && property.value == value
        }) {
            return;
        }
        self.properties.push(PresentationProperty {
            subject: subject.into(),
            name: name.into(),
            value,
        });
    }
}

impl PatchbayPresentation {
    pub fn to_portable(
        &self,
        body: &Body,
        wake: &Wake,
    ) -> Result<Presentation, PortableProjectionError> {
        body.validate()
            .map_err(PortableProjectionError::InvalidBody)?;
        wake.validate()
            .map_err(PortableProjectionError::InvalidWake)?;
        validate_lifecycle(self, body, wake)?;

        let identities = self.identities();
        let source_document_id = identities
            .source_document_id
            .ok_or(PortableProjectionError::MissingCheckedForm)?;
        let checked_form_id = identities
            .document_checked_form_id
            .ok_or(PortableProjectionError::MissingCheckedForm)?;
        let mut sign_ids = identities.sign_ids;
        sign_ids.extend(body.sign_ids.iter().cloned());
        sign_ids.extend(wake.sign_ids.iter().cloned());
        sign_ids.sort();
        sign_ids.dedup();

        let mut content = ContentBuilder::new();
        let document = content.subject_with_identity(
            format!("document/{}", source_document_id.as_str()),
            PresentationRole::Document,
            source_document_id.as_str(),
            format!("Patchbay document {}", source_document_id.as_str()),
        );
        content.line(
            &document,
            format!(
                "source={} revision={} open-form={}",
                source_document_id.as_str(),
                self.document.revision,
                self.document.open_form
            ),
        );
        append_document(self, &document, &mut content);
        append_plan_and_play(self, &document, &mut content);
        crate::portable_world_projection::append_observatory(self, &document, &mut content);
        append_sound(self, &document, &mut content);
        crate::portable_route_projection::append_routes(self, &document, &mut content);

        Presentation::new(
            self.revision,
            PresentationBasis {
                seed_id: Some(body.seed_id.clone()),
                body_id: Some(body.body_id.clone()),
                wake_id: Some(wake.wake_id.clone()),
                source_document_id: Some(source_document_id),
                checked_form_id: Some(checked_form_id),
                expanded_form_id: identities.expanded_form_id,
                plan_id: identities.plan_id,
                active_play_id: identities.active_play_id,
                sign_ids,
            },
            content.subjects,
            content.relationships,
            content.properties,
            content.text,
        )
        .map_err(PortableProjectionError::InvalidPresentation)
    }

    pub fn to_portable_front_door(
        &self,
        body: &Body,
        wake: &Wake,
        parts: &PartsView,
    ) -> Result<Presentation, PortableProjectionError> {
        if parts.body_id != body.body_id {
            return Err(PortableProjectionError::LifecycleMismatch);
        }
        let mut presentation = self.to_portable(body, wake)?;
        let mut content = ContentBuilder {
            subjects: presentation.subjects,
            relationships: presentation.relationships,
            properties: presentation.properties,
            text: presentation.text,
        };
        crate::portable_world_projection::append_body_parts(body, parts, &mut content);
        presentation.basis.sign_ids.extend(
            parts
                .parts
                .iter()
                .flat_map(|row| row.details.evidence_signs.iter().cloned()),
        );
        presentation.basis.sign_ids.extend(
            parts
                .wants_to_join
                .iter()
                .flat_map(|row| row.evidence_signs.iter().cloned()),
        );
        presentation = Presentation::new_with_semantics(
            self.revision,
            presentation.basis,
            content.subjects,
            content.relationships,
            content.properties,
            content.text,
            presentation.actions,
            presentation.disclosures,
        )
        .map_err(PortableProjectionError::InvalidPresentation)?;
        Ok(presentation)
    }
}

fn append_sound(presentation: &PatchbayPresentation, document: &str, content: &mut ContentBuilder) {
    let Some(inspection) = &presentation.sound_inspection else {
        return;
    };
    let sound = content.subject(
        PresentationRole::Plan,
        "Sound realization",
        "Sound realization compatibility and exact selection",
    );
    content.contains(document, &sound);
    content.line(
        &sound,
        format!(
            "SOUND FORM {} requirement={}",
            inspection.form.source_document_id.as_str(),
            inspection.requirement_profile_id
        ),
    );
    for candidate in &inspection.candidates {
        let status = match &candidate.status {
            conduit_observatory::SoundCandidateStatus::Compatible => "compatible".to_owned(),
            conduit_observatory::SoundCandidateStatus::Incompatible { reason } => {
                format!("incompatible:{reason}")
            }
            conduit_observatory::SoundCandidateStatus::MissingRequiredProof { required } => {
                format!("missing-proof:{required:?}")
            }
        };
        let route = match &candidate.route {
            conduit_observatory::SoundRealizationRoute::Direct => "direct".to_owned(),
            conduit_observatory::SoundRealizationRoute::Recursive { stages } => stages
                .iter()
                .map(|stage| stage.as_str())
                .collect::<Vec<_>>()
                .join(" -> "),
        };
        content.line(
            &sound,
            format!(
                "SOUND CANDIDATE {} status={} route={} implementation={} proof={:?} host={} boot={} plan={}",
                candidate.capability_id.as_str(),
                status,
                route,
                candidate.implementation_id.as_str(),
                candidate.proof_class,
                candidate.host_id.as_ref().map_or("not-selected", |id| id.as_str()),
                candidate.boot_id.as_ref().map_or("not-selected", |id| id.as_str()),
                candidate.selected_plan_id.as_ref().map_or("not-selected", |id| id.as_str()),
            ),
        );
    }
    if let Some(selected) = &inspection.selected_capability_id {
        content.line(&sound, format!("SOUND SELECTED {}", selected.as_str()));
    }
    if let Some(play) = &inspection.active_play_id {
        content.line(&sound, format!("SOUND PLAY {}", play.as_str()));
    }
}

fn validate_lifecycle(
    presentation: &PatchbayPresentation,
    body: &Body,
    wake: &Wake,
) -> Result<(), PortableProjectionError> {
    let checked = presentation
        .document
        .checked
        .forms
        .iter()
        .find(|form| form.name == presentation.document.open_form)
        .ok_or(PortableProjectionError::MissingCheckedForm)?;
    let active_body = matches!(
        &body.state,
        BodyState::Awake { wake_id } if wake_id == &wake.wake_id
    );
    if !active_body
        || body.body_id != wake.body_id
        || body.seed_id != wake.seed_id
        || body.source_document_id != wake.source_document_id
        || body.checked_form_id != wake.checked_form_id
        || presentation.document.checked.source_document_id.as_ref()
            != Some(&body.source_document_id)
        || checked.checked_form_id != body.checked_form_id
    {
        return Err(PortableProjectionError::LifecycleMismatch);
    }

    match (&presentation.plan, &presentation.play) {
        (None, None) if wake.lifecycle == WakeLifecycle::AwaitingPlan => Ok(()),
        (Some(plan), None)
            if wake.lifecycle == WakeLifecycle::AwaitingPlay
                && wake.plans.last().is_some_and(|active| {
                    active.plan_id == plan.plan_id && active.active_play_id.is_none()
                }) =>
        {
            Ok(())
        }
        (Some(plan), Some(play))
            if matches!(
                wake.lifecycle,
                WakeLifecycle::Playing | WakeLifecycle::Unsatisfied
            ) && plan.plan_id == play.plan_id
                && wake.plans.last().is_some_and(|active| {
                    active.plan_id == plan.plan_id
                        && active.active_play_id.as_ref() == Some(&play.active_play_id)
                }) =>
        {
            Ok(())
        }
        (None, Some(_)) => Err(PortableProjectionError::PlayMismatch),
        (Some(_), Some(_)) => Err(PortableProjectionError::PlayMismatch),
        _ => Err(PortableProjectionError::PlanMismatch),
    }
}

fn append_document(
    presentation: &PatchbayPresentation,
    document: &str,
    content: &mut ContentBuilder,
) {
    for form in &presentation.document.checked.forms {
        let form_subject = content.subject_with_identity(
            format!("form/{}", form.checked_form_id.as_str()),
            PresentationRole::Form,
            form.checked_form_id.as_str(),
            format!(
                "Form {} checked {}",
                form.name,
                form.checked_form_id.as_str()
            ),
        );
        content.contains(document, &form_subject);
        content.line(
            &form_subject,
            format!(
                "form={} checked={}",
                form.name,
                form.checked_form_id.as_str()
            ),
        );
        if presentation.document.open_form == form.name {
            if let Some(graph) = &presentation.graph {
                crate::portable_graph_projection::append_exact_graph(
                    &form_subject,
                    graph,
                    presentation.plan.as_ref(),
                    presentation.play.as_ref(),
                    content,
                );
                continue;
            }
        }
        for item in &form.items {
            let role = match item.kind {
                GraphItemKind::FaceInput | GraphItemKind::FaceOutput => PresentationRole::Port,
                GraphItemKind::StartupValue | GraphItemKind::Gear => PresentationRole::Gear,
                GraphItemKind::Cord => PresentationRole::Cord,
            };
            let subject = content.subject(role, &item.identity, &item.label);
            content.contains(&form_subject, &subject);
            content.line(
                &subject,
                format!(
                    "identity={} span={}..{} label={}",
                    item.identity, item.source_span.start, item.source_span.end, item.label
                ),
            );
        }
    }
    for diagnostic in &presentation.document.checked.diagnostics {
        append_diagnostic(document, diagnostic.code, &diagnostic.message, content);
    }
    if let Some(attempted) = &presentation.attempted_edit {
        for diagnostic in &attempted.diagnostics {
            append_diagnostic(document, diagnostic.code, &diagnostic.message, content);
        }
    }
}

fn append_diagnostic(document: &str, code: &str, message: &str, content: &mut ContentBuilder) {
    let subject = content.subject(
        PresentationRole::Diagnostic,
        code,
        format!("Diagnostic {code}"),
    );
    content.describes(&subject, document);
    content.line(&subject, format!("{code}: {message}"));
}

fn append_plan_and_play(
    presentation: &PatchbayPresentation,
    document: &str,
    content: &mut ContentBuilder,
) {
    if let Some(plan) = &presentation.plan {
        let subject = content.subject_with_identity(
            format!("plan/{}", plan.plan_id.as_str()),
            PresentationRole::Plan,
            plan.plan_id.as_str(),
            format!("Plan {}", plan.plan_id.as_str()),
        );
        content.describes(&subject, document);
        for line in &plan.lines {
            content.line(&subject, line);
        }
    }
    if let Some(play) = &presentation.play {
        let subject = content.subject_with_identity(
            format!("play/{}", play.active_play_id.as_str()),
            PresentationRole::Play,
            play.active_play_id.as_str(),
            format!("Play {}", play.active_play_id.as_str()),
        );
        content.describes(&subject, document);
        for line in &play.lines {
            content.line(&subject, line);
        }
        for sign in &play.signs {
            append_sign(document, &sign.sign_id, content);
        }
    }
}

pub(super) fn append_sign(document: &str, sign: &SignId, content: &mut ContentBuilder) {
    let subject = content.subject_with_identity(
        format!("sign/{}", sign.as_str()),
        PresentationRole::Sign,
        sign.as_str(),
        format!("Sign {}", sign.as_str()),
    );
    content.describes(&subject, document);
}

fn nonempty(value: String) -> String {
    if value.is_empty() {
        "unavailable".into()
    } else {
        value
    }
}
