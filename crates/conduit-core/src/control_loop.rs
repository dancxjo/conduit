use serde::{Deserialize, Serialize};

use crate::{
    AuthorityGrantId, BootId, BoundLink, ClueId, ConnectionId, HostId, LinkBindingId, PlanId,
    PlannedConnection,
};

/// Why the current deployed realization cannot continue. Queue pressure is
/// deliberately absent: finite pressure is an execution state, not Plan
/// unsatisfaction.
#[derive(Debug, Copy, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum PlayUnsatisfiedReason {
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

/// Minimum clue vocabulary for the observation -> planning -> realization
/// control loop. These records describe transitions; they do not perform link
/// retries, invoke a planner, install a fragment, or issue authority.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ControlLoopEvent {
    LinkBecameUnavailable {
        plan_id: PlanId,
        connection_id: ConnectionId,
        binding_id: LinkBindingId,
        observation_clue_id: ClueId,
    },
    PlayBecameUnsatisfied {
        plan_id: PlanId,
        reason: PlayUnsatisfiedReason,
        clue_id: ClueId,
    },
    PlanningRequested {
        prior_plan_id: PlanId,
        requester_host_id: HostId,
        requester_boot_id: BootId,
        authority: PlanningRequestAuthority,
        request_clue_id: ClueId,
    },
    PlanningRefused {
        prior_plan_id: PlanId,
        request_clue_id: ClueId,
        reason: PlanningRefusalReason,
        clue_id: ClueId,
    },
    PlanningSucceeded {
        prior_plan_id: PlanId,
        replacement_plan_id: PlanId,
        request_clue_id: ClueId,
        clue_id: ClueId,
    },
    PlanSuperseded {
        prior_plan_id: PlanId,
        replacement_plan_id: PlanId,
        clue_id: ClueId,
    },
    PlanRealized {
        plan_id: PlanId,
        clue_id: ClueId,
    },
    RouteSelectionChanged {
        plan_id: PlanId,
        connection_id: ConnectionId,
        previous_binding_id: Option<LinkBindingId>,
        selected_binding_id: LinkBindingId,
        observation_clue_id: ClueId,
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
                observation_clue_id,
            } => {
                nonempty(plan_id.as_str())
                    && nonempty(connection_id.as_str())
                    && nonempty(binding_id.as_str())
                    && nonempty(observation_clue_id.as_str())
            }
            Self::PlayBecameUnsatisfied {
                plan_id, clue_id, ..
            }
            | Self::PlanRealized { plan_id, clue_id } => {
                nonempty(plan_id.as_str()) && nonempty(clue_id.as_str())
            }
            Self::PlanningRequested {
                prior_plan_id,
                requester_host_id,
                requester_boot_id,
                authority,
                request_clue_id,
            } => {
                nonempty(prior_plan_id.as_str())
                    && nonempty(requester_host_id.as_str())
                    && nonempty(requester_boot_id.as_str())
                    && nonempty(request_clue_id.as_str())
                    && match authority {
                        PlanningRequestAuthority::HostLocal => true,
                        PlanningRequestAuthority::Delegated(grant_id) => {
                            nonempty(grant_id.as_str())
                        }
                    }
            }
            Self::PlanningRefused {
                prior_plan_id,
                request_clue_id,
                clue_id,
                ..
            } => {
                nonempty(prior_plan_id.as_str())
                    && nonempty(request_clue_id.as_str())
                    && nonempty(clue_id.as_str())
            }
            Self::PlanningSucceeded {
                prior_plan_id,
                replacement_plan_id,
                request_clue_id,
                clue_id,
            } => {
                nonempty(prior_plan_id.as_str())
                    && nonempty(replacement_plan_id.as_str())
                    && nonempty(request_clue_id.as_str())
                    && nonempty(clue_id.as_str())
            }
            Self::PlanSuperseded {
                prior_plan_id,
                replacement_plan_id,
                clue_id,
            } => {
                nonempty(prior_plan_id.as_str())
                    && nonempty(replacement_plan_id.as_str())
                    && nonempty(clue_id.as_str())
            }
            Self::RouteSelectionChanged {
                plan_id,
                connection_id,
                previous_binding_id,
                selected_binding_id,
                observation_clue_id,
            } => {
                nonempty(plan_id.as_str())
                    && nonempty(connection_id.as_str())
                    && previous_binding_id
                        .as_ref()
                        .is_none_or(|identity| nonempty(identity.as_str()))
                    && nonempty(selected_binding_id.as_str())
                    && nonempty(observation_clue_id.as_str())
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

    /// Checks route observation/selection clue against one exact deployed
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
/// without copying base or credential facts into the event vocabulary.
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
