use serde::{Deserialize, Serialize};

use crate::{
    AuthorityGrantId, BootId, BoundLink, ConnectionId, EvidenceId, HostId, LinkBindingId, PlanId,
    PlannedConnection,
};

/// Why the current deployed realization cannot continue. Queue pressure is
/// deliberately absent: finite pressure is an execution state, not Plan
/// unsatisfaction.
#[derive(Debug, Copy, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum DeploymentUnsatisfiedReason {
    NoAdmittedRouteReady,
    RequiredHostUnavailable,
    RequiredResourceUnavailable,
    RequiredAuthorityUnavailable,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum PlanningRefusalReason {
    NoCompatibleRealization,
    HardRequirementUnsatisfied,
    CurrentObservationUnavailable,
    AuthorityUnavailable,
    PlannerLimitExceeded,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum PlanningRequestAuthority {
    HostLocal,
    Delegated(AuthorityGrantId),
}

/// Minimum evidence vocabulary for the observation -> planning -> deployment
/// control loop. These records describe transitions; they do not perform link
/// retries, invoke a planner, install a fragment, or issue authority.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ControlLoopEvent {
    LinkBecameUnavailable {
        plan_id: PlanId,
        connection_id: ConnectionId,
        binding_id: LinkBindingId,
        observation_evidence_id: EvidenceId,
    },
    DeploymentBecameUnsatisfied {
        plan_id: PlanId,
        reason: DeploymentUnsatisfiedReason,
        evidence_id: EvidenceId,
    },
    PlanningRequested {
        prior_plan_id: PlanId,
        requester_host_id: HostId,
        requester_boot_id: BootId,
        authority: PlanningRequestAuthority,
        request_evidence_id: EvidenceId,
    },
    PlanningRefused {
        prior_plan_id: PlanId,
        request_evidence_id: EvidenceId,
        reason: PlanningRefusalReason,
        evidence_id: EvidenceId,
    },
    PlanningSucceeded {
        prior_plan_id: PlanId,
        replacement_plan_id: PlanId,
        request_evidence_id: EvidenceId,
        evidence_id: EvidenceId,
    },
    PlanSuperseded {
        prior_plan_id: PlanId,
        replacement_plan_id: PlanId,
        evidence_id: EvidenceId,
    },
    DeploymentInstalled {
        plan_id: PlanId,
        evidence_id: EvidenceId,
    },
    RouteSelectionChanged {
        plan_id: PlanId,
        connection_id: ConnectionId,
        previous_binding_id: Option<LinkBindingId>,
        selected_binding_id: LinkBindingId,
        observation_evidence_id: EvidenceId,
    },
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum ControlLoopEventError {
    EmptyIdentity,
    ReplacementReusedPlanIdentity,
    RouteSelectionDidNotChange,
    RouteEventPlanMismatch,
    RouteEventConnectionMismatch,
    RouteOutsideSealedCandidates,
}

impl ControlLoopEvent {
    pub fn validate(&self) -> Result<(), ControlLoopEventError> {
        let identities_are_nonempty = match self {
            Self::LinkBecameUnavailable {
                plan_id,
                connection_id,
                binding_id,
                observation_evidence_id,
            } => {
                nonempty(plan_id.as_str())
                    && nonempty(connection_id.as_str())
                    && nonempty(binding_id.as_str())
                    && nonempty(observation_evidence_id.as_str())
            }
            Self::DeploymentBecameUnsatisfied {
                plan_id,
                evidence_id,
                ..
            }
            | Self::DeploymentInstalled {
                plan_id,
                evidence_id,
            } => nonempty(plan_id.as_str()) && nonempty(evidence_id.as_str()),
            Self::PlanningRequested {
                prior_plan_id,
                requester_host_id,
                requester_boot_id,
                authority,
                request_evidence_id,
            } => {
                nonempty(prior_plan_id.as_str())
                    && nonempty(requester_host_id.as_str())
                    && nonempty(requester_boot_id.as_str())
                    && nonempty(request_evidence_id.as_str())
                    && match authority {
                        PlanningRequestAuthority::HostLocal => true,
                        PlanningRequestAuthority::Delegated(grant_id) => {
                            nonempty(grant_id.as_str())
                        }
                    }
            }
            Self::PlanningRefused {
                prior_plan_id,
                request_evidence_id,
                evidence_id,
                ..
            } => {
                nonempty(prior_plan_id.as_str())
                    && nonempty(request_evidence_id.as_str())
                    && nonempty(evidence_id.as_str())
            }
            Self::PlanningSucceeded {
                prior_plan_id,
                replacement_plan_id,
                request_evidence_id,
                evidence_id,
            } => {
                nonempty(prior_plan_id.as_str())
                    && nonempty(replacement_plan_id.as_str())
                    && nonempty(request_evidence_id.as_str())
                    && nonempty(evidence_id.as_str())
            }
            Self::PlanSuperseded {
                prior_plan_id,
                replacement_plan_id,
                evidence_id,
            } => {
                nonempty(prior_plan_id.as_str())
                    && nonempty(replacement_plan_id.as_str())
                    && nonempty(evidence_id.as_str())
            }
            Self::RouteSelectionChanged {
                plan_id,
                connection_id,
                previous_binding_id,
                selected_binding_id,
                observation_evidence_id,
            } => {
                nonempty(plan_id.as_str())
                    && nonempty(connection_id.as_str())
                    && previous_binding_id
                        .as_ref()
                        .is_none_or(|identity| nonempty(identity.as_str()))
                    && nonempty(selected_binding_id.as_str())
                    && nonempty(observation_evidence_id.as_str())
            }
        };
        if !identities_are_nonempty {
            return Err(ControlLoopEventError::EmptyIdentity);
        }
        match self {
            Self::PlanningSucceeded {
                prior_plan_id,
                replacement_plan_id,
                ..
            }
            | Self::PlanSuperseded {
                prior_plan_id,
                replacement_plan_id,
                ..
            } if prior_plan_id == replacement_plan_id => {
                Err(ControlLoopEventError::ReplacementReusedPlanIdentity)
            }
            Self::RouteSelectionChanged {
                previous_binding_id: Some(previous),
                selected_binding_id,
                ..
            } if previous == selected_binding_id => {
                Err(ControlLoopEventError::RouteSelectionDidNotChange)
            }
            _ => Ok(()),
        }
    }

    /// Checks route observation/selection evidence against one exact deployed
    /// connection. The event may name only a route sealed by that same Plan.
    pub fn validate_route_event(
        &self,
        active_plan_id: &PlanId,
        connection: &PlannedConnection,
    ) -> Result<(), ControlLoopEventError> {
        self.validate()?;
        let (plan_id, connection_id, binding_id) = match self {
            Self::LinkBecameUnavailable {
                plan_id,
                connection_id,
                binding_id,
                ..
            } => (plan_id, connection_id, binding_id),
            Self::RouteSelectionChanged {
                plan_id,
                connection_id,
                selected_binding_id,
                ..
            } => (plan_id, connection_id, selected_binding_id),
            _ => return Ok(()),
        };
        if plan_id != active_plan_id {
            return Err(ControlLoopEventError::RouteEventPlanMismatch);
        }
        if connection_id != &connection.connection_id {
            return Err(ControlLoopEventError::RouteEventConnectionMismatch);
        }
        let admitted = connection.route_candidates.iter().any(|candidate| {
            &candidate.binding_id == binding_id && connection.permits_bound_link(candidate)
        }) || connection.route_candidates.is_empty()
            && connection
                .link_binding
                .as_ref()
                .is_some_and(|binding| &binding.binding_id == binding_id);
        if !admitted {
            return Err(ControlLoopEventError::RouteOutsideSealedCandidates);
        }
        Ok(())
    }
}

fn nonempty(value: &str) -> bool {
    !value.is_empty()
}

/// Identity-only helper for presentations that need to render a selected route
/// without copying provider or credential facts into the event vocabulary.
pub fn selected_bound_link<'a>(
    event: &ControlLoopEvent,
    connection: &'a PlannedConnection,
) -> Option<&'a BoundLink> {
    let ControlLoopEvent::RouteSelectionChanged {
        selected_binding_id,
        ..
    } = event
    else {
        return None;
    };
    connection
        .route_candidates
        .iter()
        .find(|candidate| &candidate.binding_id == selected_binding_id)
}
