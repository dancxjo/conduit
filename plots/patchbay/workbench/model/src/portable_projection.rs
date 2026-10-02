//! Adapt the living Patchbay projection into Conduit's portable Presentation value.

use conduit_body::{Body, BodyLifecycleError, BodyState, Wake, WakeLifecycle};
use conduit_core::{ActivePlayId, CheckedPlotId, ExpandedPlotId, PlanId, SignId, SourceDocumentId};
use conduit_presentation::{
    Presentation, PresentationAction, PresentationActionAvailability, PresentationBasis,
    PresentationContributionBasis, PresentationDisclosure, PresentationDisclosureLevel,
    PresentationError, PresentationFragment, PresentationFragmentError, PresentationRole,
};

pub(super) use crate::portable_content::ContentBuilder;
use crate::{GraphItemKind, PatchbayPresentation};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PortableProjectionError {
    InvalidBody(BodyLifecycleError),
    InvalidWake(BodyLifecycleError),
    LifecycleMismatch,
    MissingCheckedPlot,
    PlanMismatch,
    PlayMismatch,
    InvalidPresentation(PresentationError),
    InvalidFragment(PresentationFragmentError),
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

impl PatchbayPresentation {
    pub fn to_portable(
        &self,
        body: &Body,
        wake: &Wake,
    ) -> Result<Presentation, PortableProjectionError> {
        self.to_portable_with_wake(body, Some(wake))
    }

    pub(super) fn to_portable_with_wake(
        &self,
        body: &Body,
        wake: Option<&Wake>,
    ) -> Result<Presentation, PortableProjectionError> {
        let projection = self.prepare_portable_content(body, wake)?;
        Presentation::new_with_semantics(
            self.revision,
            PresentationBasis {
                body_id: Some(body.body_id.clone()),
                wake_id: wake.map(|wake| wake.wake_id.clone()),
                source_document_id: Some(projection.source_document_id),
                checked_plot_id: Some(projection.checked_plot_id),
                expanded_plot_id: projection.expanded_plot_id,
                plan_id: projection.plan_id,
                active_play_id: projection.active_play_id,
                sign_ids: projection.sign_ids,
            },
            projection.content.subjects,
            projection.content.relationships,
            projection.content.properties,
            projection.content.text,
            projection.actions,
            vec![PresentationDisclosure {
                subject: projection.document,
                level: PresentationDisclosureLevel::Primary,
            }],
        )
        .map_err(PortableProjectionError::InvalidPresentation)
    }

    /// Project Patchbay meaning as an ordinary resident Plot contribution.
    ///
    /// Face-owned Body and resident-Plot subjects are referenced but not
    /// redeclared, preventing Patchbay from becoming a second truth registry.
    pub fn to_presentation_fragment(
        &self,
        body: &Body,
        wake: &Wake,
        basis: PresentationContributionBasis,
    ) -> Result<PresentationFragment, PortableProjectionError> {
        let mut projection = self.prepare_portable_content(body, Some(wake))?;
        let face_owned_plots = body
            .workset
            .plots()
            .iter()
            .map(|plot| format!("plot/{}", plot.checked_plot_id.as_str()))
            .collect::<Vec<_>>();
        let body_subject = format!("body/{}", body.body_id.as_str());
        projection.content.subjects.retain(|subject| {
            subject.identity != body_subject && !face_owned_plots.contains(&subject.identity)
        });
        let fragment = PresentationFragment {
            basis,
            subjects: projection.content.subjects,
            relationships: projection.content.relationships,
            composition: Vec::new(),
            properties: projection.content.properties,
            text: projection.content.text,
            actions: projection.actions,
            disclosures: vec![PresentationDisclosure {
                subject: projection.document,
                level: PresentationDisclosureLevel::Primary,
            }],
            temporal_references: Vec::new(),
            temporal_facts: Vec::new(),
        };
        fragment
            .validate_bounds()
            .map_err(PortableProjectionError::InvalidFragment)?;
        Ok(fragment)
    }

    fn prepare_portable_content(
        &self,
        body: &Body,
        wake: Option<&Wake>,
    ) -> Result<PreparedPatchbayContent, PortableProjectionError> {
        body.validate()
            .map_err(PortableProjectionError::InvalidBody)?;
        if let Some(wake) = wake {
            wake.validate()
                .map_err(PortableProjectionError::InvalidWake)?;
            validate_lifecycle(self, body, wake)?;
        } else if body.state != BodyState::Lulled || self.plan.is_some() || self.play.is_some() {
            return Err(PortableProjectionError::LifecycleMismatch);
        }

        let identities = self.identities();
        let source_document_id = identities
            .source_document_id
            .ok_or(PortableProjectionError::MissingCheckedPlot)?;
        let checked_plot_id = identities
            .document_checked_plot_id
            .ok_or(PortableProjectionError::MissingCheckedPlot)?;
        let mut sign_ids = identities.sign_ids;
        sign_ids.extend(body.sign_ids.iter().cloned());
        if let Some(wake) = wake {
            sign_ids.extend(wake.sign_ids.iter().cloned());
        }
        sign_ids.sort();
        sign_ids.dedup();

        let mut content = ContentBuilder::new();
        let document = content.subject_with_identity(
            format!("document/{}", source_document_id.as_str()),
            PresentationRole::Document,
            format!("Patchbay document {}", source_document_id.as_str()),
        );
        content.line(
            &document,
            format!(
                "source={} revision={} open-plot={}",
                source_document_id.as_str(),
                self.document.revision,
                self.document.open_plot
            ),
        );
        append_document(self, &document, &mut content);
        append_plan_and_play(self, &document, &mut content);
        crate::portable_world_projection::append_observatory(self, &document, &mut content);
        append_sound(self, &document, &mut content);
        crate::portable_route_projection::append_routes(self, &document, &mut content);

        let target = identities
            .expanded_plot_id
            .as_ref()
            .map_or_else(|| document.clone(), |identity| identity.as_str().to_owned());
        if target != document {
            let action_target = content.subject_with_identity(
                target.clone(),
                PresentationRole::Plot,
                "Current checked and expanded Plot",
            );
            content.contains(&document, &action_target);
        }
        let actions = wake.map_or_else(
            || lifecycle_actions(WakeLifecycle::Lulled, &target),
            |wake| lifecycle_actions(wake.lifecycle, &target),
        );
        Ok(PreparedPatchbayContent {
            source_document_id,
            checked_plot_id,
            expanded_plot_id: identities.expanded_plot_id,
            plan_id: identities.plan_id,
            active_play_id: identities.active_play_id,
            sign_ids,
            document,
            content,
            actions,
        })
    }
}

struct PreparedPatchbayContent {
    source_document_id: SourceDocumentId,
    checked_plot_id: CheckedPlotId,
    expanded_plot_id: Option<ExpandedPlotId>,
    plan_id: Option<PlanId>,
    active_play_id: Option<ActivePlayId>,
    sign_ids: Vec<SignId>,
    document: String,
    content: ContentBuilder,
    actions: Vec<PresentationAction>,
}

pub(super) fn lifecycle_actions(lifecycle: WakeLifecycle, target: &str) -> Vec<PresentationAction> {
    let current: &[&str] = match lifecycle {
        WakeLifecycle::AwaitingPlan
        | WakeLifecycle::Unsatisfied
        | WakeLifecycle::AwaitingReplacement => &["plan", "lull"],
        WakeLifecycle::AwaitingPlay => &["play", "lull"],
        WakeLifecycle::Playing => &["stop", "hold"],
        WakeLifecycle::Held | WakeLifecycle::Failed => &["lull"],
        WakeLifecycle::Lulled => &["wake"],
    };
    [
        ("open-back", "Open", "conduit.intent/open@1"),
        ("save", "Save", "conduit.intent/save@1"),
        (
            "toggle-linear-view",
            "Toggle linear view",
            "conduit.intent/toggle-linear-view@1",
        ),
        ("birth", "Birth", "conduit.intent/birth@1"),
        ("wake", "Wake", "conduit.intent/wake@1"),
        ("plan", "Plan", "conduit.intent/plan@1"),
        ("play", "Play", "conduit.intent/play@1"),
        ("hold", "Hold", "conduit.intent/hold@1"),
        ("stop", "Stop", "conduit.intent/stop@1"),
        ("lull", "Lull", "conduit.intent/lull@1"),
    ]
    .iter()
    .map(|(name, label, intent)| PresentationAction {
        identity: format!("action/{name}/{target}"),
        intent: (*intent).into(),
        target: target.into(),
        name: (*label).into(),
        arguments: vec![],
        disclosure: PresentationDisclosureLevel::CurrentAction,
        availability: if matches!(*name, "open-back" | "save" | "toggle-linear-view")
            || current.contains(name)
        {
            PresentationActionAvailability::Available
        } else {
            PresentationActionAvailability::Unavailable {
                reason_code: "lifecycle/not-current".into(),
                explanation: format!("{label} is not available in {lifecycle:?}."),
            }
        },
    })
    .collect()
}

fn append_sound(presentation: &PatchbayPresentation, document: &str, content: &mut ContentBuilder) {
    let Some(inspection) = &presentation.sound_inspection else {
        return;
    };
    let sound = content.subject(
        PresentationRole::Plan,
        "Sound realization compatibility and exact selection",
    );
    content.contains(document, &sound);
    content.line(
        &sound,
        format!(
            "SOUND PLOT {} requirement={}",
            inspection.plot.source_document_id.as_str(),
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
                .join(" > "),
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
        .plots
        .iter()
        .find(|plot| plot.name == presentation.document.open_plot)
        .ok_or(PortableProjectionError::MissingCheckedPlot)?;
    let active_body = matches!(
        &body.state,
        BodyState::Awake { wake_id } if wake_id == &wake.wake_id
    );
    if !active_body
        || body.body_id != wake.body_id
        || body.workset != wake.workset
        || body.workload_revision != wake.workload_revision
        || !body.workset.contains(&conduit_body::ResidentPlot::new(
            presentation
                .document
                .checked
                .source_document_id
                .clone()
                .ok_or(PortableProjectionError::MissingCheckedPlot)?,
            checked.checked_plot_id.clone(),
        ))
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
    for plot in &presentation.document.checked.plots {
        let plot_subject = content.subject_with_identity(
            format!("plot/{}", plot.checked_plot_id.as_str()),
            PresentationRole::Plot,
            format!(
                "Plot {} checked {}",
                plot.name,
                plot.checked_plot_id.as_str()
            ),
        );
        content.contains(document, &plot_subject);
        content.line(
            &plot_subject,
            format!(
                "plot={} checked={}",
                plot.name,
                plot.checked_plot_id.as_str()
            ),
        );
        if presentation.document.open_plot == plot.name {
            if let Some(graph) = &presentation.graph {
                crate::portable_graph_projection::append_exact_graph(
                    &plot_subject,
                    graph,
                    presentation.plan.as_ref(),
                    presentation.play.as_ref(),
                    content,
                );
                continue;
            }
        }
        for item in &plot.items {
            let role = match item.kind {
                GraphItemKind::FaceInput | GraphItemKind::FaceOutput => PresentationRole::Port,
                GraphItemKind::StartupValue | GraphItemKind::Gear => PresentationRole::Gear,
                GraphItemKind::Cord => PresentationRole::Cord,
            };
            let subject = content.subject(role, &item.label);
            content.contains(&plot_subject, &subject);
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
    let subject = content.subject(PresentationRole::Diagnostic, format!("Diagnostic {code}"));
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
        format!("Sign {}", sign.as_str()),
    );
    content.describes(&subject, document);
}
