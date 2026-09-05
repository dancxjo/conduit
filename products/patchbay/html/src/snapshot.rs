use crate::transport_types::RendererSnapshot;
use conduit_core::SignId;
use conduit_presentation::{
    project_model_temporal_context, ManifestationFailure, ManifestationLifecycle,
};
use patchbay_model::RendererExecution;

pub const SNAPSHOT_SCHEMA: &str = "conduit.patchbay.portable-presentation";
pub const MAX_NAVIGATION_SNAPSHOT_BYTES: usize = 512 * 1024;
pub const MAX_SNAPSHOT_BYTES: usize = 2 * conduit_presentation::MAX_PRESENTATION_TOTAL_BYTES
    + MAX_NAVIGATION_SNAPSHOT_BYTES
    + 131_072;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SnapshotError {
    Oversized,
    Malformed(String),
    UnsupportedSchema,
    Stale { minimum: u64, offered: u64 },
    InvalidIdentity,
}

impl std::fmt::Display for SnapshotError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Oversized => {
                formatter.write_str("renderer snapshot exceeds its finite byte bound")
            }
            Self::Malformed(message) => write!(formatter, "malformed renderer snapshot: {message}"),
            Self::UnsupportedSchema => formatter.write_str("unsupported snapshot schema"),
            Self::Stale { minimum, offered } => {
                write!(
                    formatter,
                    "stale renderer revision {offered}; minimum is {minimum}"
                )
            }
            Self::InvalidIdentity => {
                formatter.write_str("portable Presentation identity is invalid")
            }
        }
    }
}

impl std::error::Error for SnapshotError {}

impl RendererSnapshot {
    pub fn navigation_observation(
        &self,
    ) -> Result<Option<conduit_presentation::NavigationObservation>, SnapshotError> {
        self.navigation
            .as_ref()
            .map(|navigation| {
                conduit_presentation::observe_navigation(
                    &self.presentation,
                    &navigation.navigation,
                    &navigation.projection,
                    &navigation.cursor,
                )
                .map_err(|_| SnapshotError::InvalidIdentity)
            })
            .transpose()
    }

    pub fn from_execution(execution: RendererExecution) -> Result<Self, SnapshotError> {
        execution
            .validate()
            .map_err(|_| SnapshotError::InvalidIdentity)?;
        let entrance = patchbay_model::PatchbayEntranceState::enter(&execution.presentation)
            .map_err(|_| SnapshotError::InvalidIdentity)?;
        let temporal_context = project_model_temporal_context(&execution.presentation)
            .map_err(|_| SnapshotError::InvalidIdentity)?;
        let value = Self {
            schema: SNAPSHOT_SCHEMA.into(),
            revision: execution.presentation.revision,
            renderer: execution
                .self_inspection()
                .map_err(|_| SnapshotError::InvalidIdentity)?,
            presentation: execution.presentation,
            temporal_context,
            navigation: None,
            entrance,
            parts: None,
            authoring: None,
            body_workbench: None,
            body_host_offer_evidence: None,
            body_host_planning_offer: None,
            debugger: None,
            watches: None,
            timeline: None,
            timeline_projection: None,
            debugger_control: None,
            interaction: crate::HtmlInteractionState::default(),
        };
        value.validate()?;
        Ok(value)
    }

    pub fn mark_available(&mut self, sign_id: SignId) -> Result<(), SnapshotError> {
        self.renderer.manifestation = self
            .renderer
            .manifestation
            .transition(ManifestationLifecycle::Available, sign_id)
            .map_err(|_| SnapshotError::InvalidIdentity)?;
        self.validate()
    }

    pub fn attach_parts(&mut self, parts: patchbay_model::PartsView) -> Result<(), SnapshotError> {
        if self.presentation.basis.body_id.as_ref() != Some(&parts.body_id) {
            return Err(SnapshotError::InvalidIdentity);
        }
        self.parts = Some(parts);
        self.validate()
    }

    pub fn attach_navigation(
        &mut self,
        navigation: patchbay_model::PatchbayNavigationProjection,
    ) -> Result<(), SnapshotError> {
        navigation
            .navigation
            .validate(&self.presentation)
            .map_err(|_| SnapshotError::InvalidIdentity)?;
        navigation
            .projection
            .project(
                &self.presentation,
                &navigation.navigation,
                &navigation.cursor,
            )
            .map_err(|_| SnapshotError::InvalidIdentity)?;
        self.navigation = Some(navigation);
        self.validate()
    }

    pub fn attach_authoring(
        &mut self,
        authoring: crate::transport_types::BrowserAuthoring,
    ) -> Result<(), SnapshotError> {
        if authoring.palette.len() > crate::transport_types::MAX_BROWSER_PALETTE_ENTRIES
            || authoring.source_document_id.is_empty()
            || authoring.expanded_form_id.is_empty()
        {
            return Err(SnapshotError::InvalidIdentity);
        }
        self.authoring = Some(authoring);
        self.validate()
    }

    pub fn attach_body_workbench(
        &mut self,
        workbench: crate::BrowserBodyWorkbench,
    ) -> Result<(), SnapshotError> {
        self.body_workbench = Some(workbench);
        self.validate()
    }

    pub fn attach_debugger(
        &mut self,
        debugger: patchbay_model::DebuggerPresentation,
    ) -> Result<(), SnapshotError> {
        self.debugger = Some(debugger);
        self.validate()
    }

    pub fn attach_watches(
        &mut self,
        watches: patchbay_model::DebuggerWatchSet,
    ) -> Result<(), SnapshotError> {
        self.watches = Some(watches);
        self.validate()
    }

    pub fn attach_timeline(
        &mut self,
        timeline: patchbay_model::DebuggerTimeline,
    ) -> Result<(), SnapshotError> {
        self.timeline_projection = Some(timeline.project(self.watches.as_ref()));
        self.timeline = Some(timeline);
        self.validate()
    }

    pub fn attach_debugger_control(
        &mut self,
        control: patchbay_model::DebuggerExecutionControl,
    ) -> Result<(), SnapshotError> {
        self.debugger_control = Some(control);
        self.validate()
    }

    pub fn mark_failed(
        &mut self,
        failure: ManifestationFailure,
        sign_id: SignId,
    ) -> Result<(), SnapshotError> {
        self.renderer.manifestation = self
            .renderer
            .manifestation
            .fail(failure, sign_id)
            .map_err(|_| SnapshotError::InvalidIdentity)?;
        self.validate()
    }

    pub fn mark_closed(&mut self, sign_id: SignId) -> Result<(), SnapshotError> {
        self.renderer.manifestation = self
            .renderer
            .manifestation
            .transition(ManifestationLifecycle::Closed, sign_id)
            .map_err(|_| SnapshotError::InvalidIdentity)?;
        self.validate()
    }

    pub fn encode(&self) -> Result<Vec<u8>, SnapshotError> {
        self.validate()?;
        let bytes = serde_json::to_vec(self)
            .map_err(|error| SnapshotError::Malformed(error.to_string()))?;
        if bytes.len() > MAX_SNAPSHOT_BYTES {
            return Err(SnapshotError::Oversized);
        }
        Ok(bytes)
    }

    pub fn decode(bytes: &[u8], minimum_revision: u64) -> Result<Self, SnapshotError> {
        if bytes.len() > MAX_SNAPSHOT_BYTES {
            return Err(SnapshotError::Oversized);
        }
        let value: Self = serde_json::from_slice(bytes)
            .map_err(|error| SnapshotError::Malformed(error.to_string()))?;
        if value.revision < minimum_revision {
            return Err(SnapshotError::Stale {
                minimum: minimum_revision,
                offered: value.revision,
            });
        }
        value.validate()?;
        Ok(value)
    }

    fn validate(&self) -> Result<(), SnapshotError> {
        let invalid_parts = self.parts.as_ref().is_some_and(|parts| {
            self.presentation.basis.body_id.as_ref() != Some(&parts.body_id)
                || parts.parts.len() > patchbay_model::MAX_PARTS_VIEW_ROWS
                || parts.wants_to_join.len() > patchbay_model::MAX_WANTS_TO_JOIN_ROWS
        });
        let invalid_navigation = self.navigation_observation().is_err();
        let invalid_authoring = self.authoring.as_ref().is_some_and(|authoring| {
            authoring.palette.len() > crate::transport_types::MAX_BROWSER_PALETTE_ENTRIES
                || authoring.source_document_id.is_empty()
                || authoring.expanded_form_id.is_empty()
        });
        let invalid_temporal_context = project_model_temporal_context(&self.presentation)
            .map_or(true, |expected| expected != self.temporal_context);
        let invalid_workbench = self.body_workbench.as_ref().is_some_and(|workbench| {
            crate::body_workbench::validate_body_workbench(workbench, &self.presentation).is_err()
        });
        let invalid_body_host_offer = self
            .body_host_offer_evidence
            .as_ref()
            .is_some_and(crate::server::body_host_offer_evidence::invalid_projection);
        let invalid_body_host_planning_offer = self
            .body_host_planning_offer
            .as_ref()
            .is_some_and(crate::server::body_host_planning_offer::invalid_projection);
        let invalid_debugger = self.debugger.as_ref().is_some_and(|debugger| {
            debugger.schema != patchbay_model::DEBUGGER_PRESENTATION_SCHEMA
                || debugger.activities.len() > patchbay_model::MAX_DEBUGGER_SUBJECTS
                || debugger.activities.iter().any(|activity| {
                    !self
                        .presentation
                        .subjects
                        .iter()
                        .any(|subject| subject.identity == activity.subject)
                        || activity.line_subject.as_ref().is_some_and(|line| {
                            !self
                                .presentation
                                .subjects
                                .iter()
                                .any(|subject| &subject.identity == line)
                        })
                })
        });
        let invalid_watches = self.watches.as_ref().is_some_and(|watches| {
            watches.schema != patchbay_model::DEBUGGER_WATCH_SCHEMA
                || watches.watches.len() > patchbay_model::MAX_DEBUGGER_WATCHES
                || watches.eligible_subjects.len() > patchbay_model::MAX_DEBUGGER_SUBJECTS
                || self.debugger.as_ref().map(|debugger| &debugger.execution)
                    != Some(&watches.execution)
                || watches.focused_subject.as_ref().is_some_and(|focused| {
                    !watches
                        .watches
                        .iter()
                        .any(|watch| &watch.subject == focused)
                })
                || watches.watches.iter().any(|watch| {
                    watch.history.len() > patchbay_model::MAX_WATCH_HISTORY_RECORDS
                        || watch.learned_projections.len()
                            > patchbay_model::MAX_LEARNED_WATCH_PROJECTIONS
                        || watch
                            .learned_projections
                            .iter()
                            .any(|projection| projection.validate().is_err())
                        || watch.learned_projections.iter().any(|projection| {
                            watch.latest.as_ref().map(|entry| entry.sequence)
                                != Some(projection.observation_sequence)
                        })
                        || watch.execution != watches.execution
                        || !self.presentation.subjects.iter().any(|subject| {
                            subject.identity == watch.subject
                                && subject.role
                                    == match watch.role {
                                        patchbay_model::DebuggerWatchSubjectRole::Gear => {
                                            conduit_presentation::PresentationRole::Gear
                                        }
                                        patchbay_model::DebuggerWatchSubjectRole::Port => {
                                            conduit_presentation::PresentationRole::Port
                                        }
                                        patchbay_model::DebuggerWatchSubjectRole::Cord => {
                                            conduit_presentation::PresentationRole::Cord
                                        }
                                    }
                        })
                        || watch.latest.as_ref() != watch.history.last()
                })
        });
        let invalid_timeline = match (&self.timeline, &self.timeline_projection) {
            (None, None) => false,
            (Some(timeline), Some(projection)) => {
                timeline.schema != patchbay_model::DEBUGGER_TIMELINE_SCHEMA
                    || timeline.events.len() > patchbay_model::MAX_DEBUGGER_TIMELINE_EVENTS
                    || timeline.retained_bytes > patchbay_model::MAX_DEBUGGER_TIMELINE_BYTES
                    || timeline.retained_bytes
                        != timeline
                            .events
                            .iter()
                            .map(patchbay_model::DebuggerTimelineEvent::retained_bytes)
                            .sum::<usize>()
                    || timeline
                        .cursor
                        .is_some_and(|cursor| cursor >= timeline.events.len())
                    || timeline
                        .selected_event
                        .is_some_and(|cursor| cursor >= timeline.events.len())
                    || timeline.subject_filter.as_ref().is_some_and(|subject| {
                        !timeline.events.iter().any(|event| {
                            &event.subject == subject
                                || event.related_subject.as_ref() == Some(subject)
                        })
                    })
                    || timeline.events.iter().any(|event| {
                        !self
                            .presentation
                            .subjects
                            .iter()
                            .any(|subject| subject.identity == event.subject)
                            || event.value.as_ref().is_some_and(|value| {
                                value.summary.len() > patchbay_model::MAX_DEBUGGER_SUMMARY_BYTES
                            })
                            || event.related_subject.as_ref().is_some_and(|related| {
                                !self
                                    .presentation
                                    .subjects
                                    .iter()
                                    .any(|subject| &subject.identity == related)
                            })
                    })
                    || timeline.trace.as_ref().is_some_and(|trace| {
                        trace.steps.len() > patchbay_model::MAX_DEBUGGER_TIMELINE_EVENTS
                            || trace.missing_parent_sequences.len()
                                > patchbay_model::MAX_DEBUGGER_TIMELINE_EVENTS
                            || trace.steps.iter().any(|step| {
                                timeline.events.get(step.event_index).is_none_or(|event| {
                                    event.execution != trace.execution
                                        || event.sequence != step.sequence
                                        || event.subject != step.subject
                                        || event.event != step.event
                                })
                            })
                    })
                    || &timeline.project(self.watches.as_ref()) != projection
            }
            _ => true,
        };
        let invalid_debugger_control = self.debugger_control.as_ref().is_some_and(|control| {
            control.schema != patchbay_model::DEBUGGER_CONTROL_SCHEMA
                || control.eligible_subjects.is_empty()
                || control.eligible_subjects.len()
                    > patchbay_model::MAX_DEBUGGER_BREAKPOINT_SUBJECTS
                || (control.state != patchbay_model::DebuggerExecutionControlState::Stale
                    && self.debugger.as_ref().map(|debugger| &debugger.execution)
                        != Some(&control.execution))
                || control.eligible_subjects.iter().any(|identity| {
                    !self.presentation.subjects.iter().any(|subject| {
                        &subject.identity == identity
                            && subject.role == conduit_presentation::PresentationRole::Gear
                    })
                })
                || control.reason.as_ref().is_some_and(|reason| {
                    reason.len() > patchbay_model::MAX_DEBUGGER_CONTROL_REASON_BYTES
                })
                || control
                    .breakpoint_subject
                    .as_ref()
                    .is_some_and(|subject| !control.eligible_subjects.contains(subject))
                || control
                    .suspended_subject
                    .as_ref()
                    .is_some_and(|subject| !control.eligible_subjects.contains(subject))
                || (control.state == patchbay_model::DebuggerExecutionControlState::Suspended)
                    != control.suspended_subject.is_some()
                || (control.state == patchbay_model::DebuggerExecutionControlState::Stale
                    && control.reason.is_none())
        });
        if self.schema != SNAPSHOT_SCHEMA {
            return Err(SnapshotError::UnsupportedSchema);
        }
        if self.revision != self.presentation.revision
            || self.presentation.validate().is_err()
            || self.renderer.validate_against(&self.presentation).is_err()
            || self.entrance.body_id != self.presentation.basis.body_id
            || self.entrance.presentation_id != self.presentation.identity.as_str()
            || self.entrance.presentation_revision != self.presentation.revision
            || invalid_parts
            || invalid_navigation
            || invalid_authoring
            || invalid_temporal_context
            || invalid_workbench
            || invalid_body_host_offer
            || invalid_body_host_planning_offer
            || invalid_debugger
            || invalid_watches
            || invalid_timeline
            || invalid_debugger_control
        {
            return Err(SnapshotError::InvalidIdentity);
        }
        Ok(())
    }
}
