//! Adapt the living Patchbay projection into Conduit's portable Presentation value.

use conduit_body::{Body, BodyLifecycleError, BodyState, Wake, WakeLifecycle};
use conduit_core::SignId;
use conduit_presentation::{
    Presentation, PresentationBasis, PresentationError, PresentationProperty,
    PresentationPropertyValue, PresentationRelationship, PresentationRelationshipKind,
    PresentationRole, PresentationSubject, PresentationText,
};

use crate::{GraphItemKind, PatchbayPresentation};

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
    subjects: Vec<PresentationSubject>,
    pub(super) relationships: Vec<PresentationRelationship>,
    properties: Vec<PresentationProperty>,
    text: Vec<PresentationText>,
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

    fn describes(&mut self, source: &str, target: &str) {
        self.relationships.push(PresentationRelationship {
            source: source.into(),
            target: target.into(),
            kind: PresentationRelationshipKind::Describes,
        });
    }

    fn line(&mut self, subject: &str, value: impl Into<String>) {
        self.text.push(PresentationText {
            subject: subject.into(),
            text: nonempty(value.into()),
        });
    }

    pub(super) fn property(&mut self, subject: &str, name: &str, value: PresentationPropertyValue) {
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
        let document = content.subject(
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
        append_topology(self, &document, &mut content);
        append_routes(self, &document, &mut content);

        Presentation::new(
            self.revision,
            PresentationBasis {
                seed_id: body.seed_id.clone(),
                body_id: body.body_id.clone(),
                wake_id: wake.wake_id.clone(),
                source_document_id,
                checked_form_id,
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
        let form_subject = content.subject(
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
                crate::portable_graph_projection::append_exact_graph(&form_subject, graph, content);
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
        let subject = content.subject(
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
        let subject = content.subject(
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

fn append_topology(
    presentation: &PatchbayPresentation,
    document: &str,
    content: &mut ContentBuilder,
) {
    let Some(topology) = &presentation.topology else {
        return;
    };
    for host in &topology.hosts {
        let subject = content.subject(
            PresentationRole::Host,
            host.host_id.as_str(),
            format!(
                "Host {} boot {}",
                host.host_id.as_str(),
                host.boot_id.as_str()
            ),
        );
        content.describes(&subject, document);
    }
    for sign in &topology.signs {
        append_sign(document, &sign.sign_id, content);
    }
}

fn append_routes(
    presentation: &PatchbayPresentation,
    document: &str,
    content: &mut ContentBuilder,
) {
    for route in &presentation.routes {
        let subject = content.subject(
            PresentationRole::Route,
            route.same_plan.plan.connection_id.as_str(),
            format!(
                "Route {} under Plan {}",
                route.same_plan.plan.connection_id.as_str(),
                route.same_plan.plan.plan_id.as_str()
            ),
        );
        content.describes(&subject, document);
        for line in route.linear_lines() {
            content.line(&subject, line);
        }
        append_line_candidates(&subject, "prior", &route.new_plan.prior, content);
        append_line_candidates(&subject, "same-plan", &route.same_plan.plan, content);
    }
}

fn append_line_candidates(
    route: &str,
    phase: &str,
    plan: &crate::RoutePlanPresentation,
    content: &mut ContentBuilder,
) {
    for candidate in &plan.candidates {
        let subject = content.subject(
            PresentationRole::Cord,
            candidate.binding_id.as_str(),
            format!(
                "Route candidate {} in Plan {}",
                candidate.binding_id.as_str(),
                plan.plan_id.as_str()
            ),
        );
        content.contains(route, &subject);
        content.property(
            &subject,
            "phase",
            PresentationPropertyValue::Text(phase.into()),
        );
        content.property(
            &subject,
            "plan-id",
            PresentationPropertyValue::Identity(plan.plan_id.as_str().into()),
        );
        content.property(
            &subject,
            "connection-id",
            PresentationPropertyValue::Identity(plan.connection_id.as_str().into()),
        );
        content.property(
            &subject,
            "binding-id",
            PresentationPropertyValue::Identity(candidate.binding_id.as_str().into()),
        );
        content.property(
            &subject,
            "base",
            PresentationPropertyValue::ConnectionBase(candidate.base),
        );
        content.property(
            &subject,
            "base-instance-id",
            PresentationPropertyValue::Identity(candidate.base_instance_id.as_str().into()),
        );
        content.property(
            &subject,
            "order",
            PresentationPropertyValue::Count(candidate.order as u64),
        );
    }
}

fn append_sign(document: &str, sign: &SignId, content: &mut ContentBuilder) {
    let subject = content.subject(
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
