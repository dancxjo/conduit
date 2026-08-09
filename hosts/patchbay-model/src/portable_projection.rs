//! Adapt the living Patchbay projection into Conduit's portable Presentation value.

use conduit_core::EvidenceId;
use conduit_presentation::{
    Presentation, PresentationBasis, PresentationError, PresentationProperty,
    PresentationPropertyValue, PresentationRelationship, PresentationRelationshipKind,
    PresentationRole, PresentationSubject, PresentationText,
};
use conduit_realm::{
    ActivationLifecycle, DeploymentState, RealmActivation, RealmDeployment, RealmLifecycleError,
};

use crate::{GraphItemKind, PatchbayPresentation};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PortableProjectionError {
    InvalidDeployment(RealmLifecycleError),
    InvalidActivation(RealmLifecycleError),
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

struct ContentBuilder {
    subjects: Vec<PresentationSubject>,
    relationships: Vec<PresentationRelationship>,
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

    fn subject(
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

    fn contains(&mut self, source: &str, target: &str) {
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

    fn property(&mut self, subject: &str, name: &str, value: PresentationPropertyValue) {
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
        deployment: &RealmDeployment,
        activation: &RealmActivation,
    ) -> Result<Presentation, PortableProjectionError> {
        deployment
            .validate()
            .map_err(PortableProjectionError::InvalidDeployment)?;
        activation
            .validate()
            .map_err(PortableProjectionError::InvalidActivation)?;
        validate_lifecycle(self, deployment, activation)?;

        let identities = self.identities();
        let source_document_id = identities
            .source_document_id
            .ok_or(PortableProjectionError::MissingCheckedForm)?;
        let checked_form_id = identities
            .document_checked_form_id
            .ok_or(PortableProjectionError::MissingCheckedForm)?;
        let mut evidence_ids = identities.evidence_ids;
        evidence_ids.extend(deployment.evidence_ids.iter().cloned());
        evidence_ids.extend(activation.evidence_ids.iter().cloned());
        evidence_ids.sort();
        evidence_ids.dedup();

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
                realm_id: deployment.realm_id.clone(),
                deployment_id: deployment.deployment_id.clone(),
                activation_id: activation.activation_id.clone(),
                source_document_id,
                checked_form_id,
                expanded_form_id: identities.expanded_form_id,
                plan_id: identities.plan_id,
                active_play_id: identities.active_play_id,
                evidence_ids,
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
    deployment: &RealmDeployment,
    activation: &RealmActivation,
) -> Result<(), PortableProjectionError> {
    let checked = presentation
        .document
        .checked
        .forms
        .iter()
        .find(|form| form.name == presentation.document.open_form)
        .ok_or(PortableProjectionError::MissingCheckedForm)?;
    let active_deployment = matches!(
        &deployment.state,
        DeploymentState::Active { activation_id } if activation_id == &activation.activation_id
    );
    if !active_deployment
        || deployment.deployment_id != activation.deployment_id
        || deployment.realm_id != activation.realm_id
        || deployment.source_document_id != activation.source_document_id
        || deployment.checked_form_id != activation.checked_form_id
        || presentation.document.checked.source_document_id.as_ref()
            != Some(&deployment.source_document_id)
        || checked.checked_form_id != deployment.checked_form_id
    {
        return Err(PortableProjectionError::LifecycleMismatch);
    }

    match (&presentation.plan, &presentation.play) {
        (None, None) if activation.lifecycle == ActivationLifecycle::AwaitingPlan => Ok(()),
        (Some(plan), None)
            if activation.lifecycle == ActivationLifecycle::AwaitingPlay
                && activation.plans.last().is_some_and(|active| {
                    active.plan_id == plan.plan_id && active.active_play_id.is_none()
                }) =>
        {
            Ok(())
        }
        (Some(plan), Some(play))
            if matches!(
                activation.lifecycle,
                ActivationLifecycle::Active | ActivationLifecycle::Unsatisfied
            ) && plan.plan_id == play.plan_id
                && activation.plans.last().is_some_and(|active| {
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
        for item in &form.items {
            let role = match item.kind {
                GraphItemKind::FaceInput | GraphItemKind::FaceOutput => PresentationRole::Port,
                GraphItemKind::StartupValue | GraphItemKind::Cell => PresentationRole::Cell,
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
        for evidence in &play.evidence {
            append_evidence(document, &evidence.evidence_id, content);
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
    for evidence in &topology.evidence {
        append_evidence(document, &evidence.evidence_id, content);
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
        append_route_candidates(&subject, "prior", &route.new_plan.prior, content);
        append_route_candidates(&subject, "same-plan", &route.same_plan.plan, content);
    }
}

fn append_route_candidates(
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
            "provider",
            PresentationPropertyValue::ConnectionProvider(candidate.provider),
        );
        content.property(
            &subject,
            "provider-instance-id",
            PresentationPropertyValue::Identity(candidate.provider_instance_id.as_str().into()),
        );
        content.property(
            &subject,
            "order",
            PresentationPropertyValue::Count(candidate.order as u64),
        );
    }
}

fn append_evidence(document: &str, evidence: &EvidenceId, content: &mut ContentBuilder) {
    let subject = content.subject(
        PresentationRole::Evidence,
        evidence.as_str(),
        format!("Evidence {}", evidence.as_str()),
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
