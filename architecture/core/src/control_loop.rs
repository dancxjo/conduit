use serde::{Deserialize, Serialize};

use crate::{
    AdmittedLine, AuthorityGrantId, BootId, ConnectionId, HostId, LineId, LinkBindingId,
    OfferGeneration, Plan, PlanId, PlannedConnection, SignId,
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

/// Minimum sign vocabulary for the observation > planning > realization
/// control loop. These records describe transitions; they do not perform link
/// retries, invoke a planner, install a fragment, or issue authority.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ControlLoopEvent {
    HostBecameUnavailable {
        plan_id: PlanId,
        host_id: HostId,
        boot_id: BootId,
        offer_generation: OfferGeneration,
        observation_sign_id: SignId,
    },
    LineBecameUnavailable {
        plan_id: PlanId,
        connection_id: ConnectionId,
        line_id: LineId,
        binding_id: LinkBindingId,
        observation_sign_id: SignId,
    },
    PlayBecameUnsatisfied {
        plan_id: PlanId,
        reason: PlayUnsatisfiedReason,
        sign_id: SignId,
    },
    PlanningRequested {
        prior_plan_id: PlanId,
        requester_host_id: HostId,
        requester_boot_id: BootId,
        authority: PlanningRequestAuthority,
        request_sign_id: SignId,
    },
    PlanningRefused {
        prior_plan_id: PlanId,
        request_sign_id: SignId,
        reason: PlanningRefusalReason,
        sign_id: SignId,
    },
    PlanningSucceeded {
        prior_plan_id: PlanId,
        replacement_plan_id: PlanId,
        request_sign_id: SignId,
        sign_id: SignId,
    },
    PlanSuperseded {
        prior_plan_id: PlanId,
        replacement_plan_id: PlanId,
        sign_id: SignId,
    },
    PlanRealized {
        plan_id: PlanId,
        sign_id: SignId,
    },
    LineSelectionChanged {
        plan_id: PlanId,
        connection_id: ConnectionId,
        previous_line_id: Option<LineId>,
        selected_line_id: LineId,
        selected_binding_id: LinkBindingId,
        observation_sign_id: SignId,
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
    HostEventPlanMismatch,
    HostOutsideSealedPlan,
    InvalidPlan,
}

impl ControlLoopEvent {
    pub fn validate(&self) -> Result<(), ControlLoopEventError> {
        let identities_are_nonempty = match self {
            Self::HostBecameUnavailable {
                plan_id,
                host_id,
                boot_id,
                observation_sign_id,
                ..
            } => {
                nonempty(plan_id.as_str())
                    && nonempty(host_id.as_str())
                    && nonempty(boot_id.as_str())
                    && nonempty(observation_sign_id.as_str())
            }
            Self::LineBecameUnavailable {
                plan_id,
                connection_id,
                line_id,
                binding_id,
                observation_sign_id,
            } => {
                nonempty(plan_id.as_str())
                    && nonempty(connection_id.as_str())
                    && nonempty(line_id.as_str())
                    && nonempty(binding_id.as_str())
                    && nonempty(observation_sign_id.as_str())
            }
            Self::PlayBecameUnsatisfied {
                plan_id, sign_id, ..
            }
            | Self::PlanRealized { plan_id, sign_id } => {
                nonempty(plan_id.as_str()) && nonempty(sign_id.as_str())
            }
            Self::PlanningRequested {
                prior_plan_id,
                requester_host_id,
                requester_boot_id,
                authority,
                request_sign_id,
            } => {
                nonempty(prior_plan_id.as_str())
                    && nonempty(requester_host_id.as_str())
                    && nonempty(requester_boot_id.as_str())
                    && nonempty(request_sign_id.as_str())
                    && match authority {
                        PlanningRequestAuthority::HostLocal => true,
                        PlanningRequestAuthority::Delegated(grant_id) => {
                            nonempty(grant_id.as_str())
                        }
                    }
            }
            Self::PlanningRefused {
                prior_plan_id,
                request_sign_id,
                sign_id,
                ..
            } => {
                nonempty(prior_plan_id.as_str())
                    && nonempty(request_sign_id.as_str())
                    && nonempty(sign_id.as_str())
            }
            Self::PlanningSucceeded {
                prior_plan_id,
                replacement_plan_id,
                request_sign_id,
                sign_id,
            } => {
                nonempty(prior_plan_id.as_str())
                    && nonempty(replacement_plan_id.as_str())
                    && nonempty(request_sign_id.as_str())
                    && nonempty(sign_id.as_str())
            }
            Self::PlanSuperseded {
                prior_plan_id,
                replacement_plan_id,
                sign_id,
            } => {
                nonempty(prior_plan_id.as_str())
                    && nonempty(replacement_plan_id.as_str())
                    && nonempty(sign_id.as_str())
            }
            Self::LineSelectionChanged {
                plan_id,
                connection_id,
                previous_line_id,
                selected_line_id,
                selected_binding_id,
                observation_sign_id,
            } => {
                nonempty(plan_id.as_str())
                    && nonempty(connection_id.as_str())
                    && previous_line_id
                        .as_ref()
                        .is_none_or(|identity| nonempty(identity.as_str()))
                    && nonempty(selected_line_id.as_str())
                    && nonempty(selected_binding_id.as_str())
                    && nonempty(observation_sign_id.as_str())
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
            Self::LineSelectionChanged {
                previous_line_id: Some(previous),
                selected_line_id,
                ..
            } if previous == selected_line_id => {
                Err(ControlLoopEventError::RouteSelectionDidNotChange)
            }
            _ => Ok(()),
        }
    }

    /// Checks route observation/selection sign against one exact deployed
    /// connection. The event may name only a route sealed by that same Plan.
    pub fn validate_route_event(
        &self,
        active_plan_id: &PlanId,
        connection: &PlannedConnection,
    ) -> Result<(), ControlLoopEventError> {
        self.validate()?;
        let (plan_id, connection_id, line_id, binding_id) = match self {
            Self::LineBecameUnavailable {
                plan_id,
                connection_id,
                line_id,
                binding_id,
                ..
            } => (plan_id, connection_id, line_id, binding_id),
            Self::LineSelectionChanged {
                plan_id,
                connection_id,
                selected_line_id,
                selected_binding_id,
                ..
            } => (
                plan_id,
                connection_id,
                selected_line_id,
                selected_binding_id,
            ),
            _ => return Ok(()),
        };
        if plan_id != active_plan_id {
            return Err(ControlLoopEventError::RouteEventPlanMismatch);
        }
        if connection_id != &connection.connection_id {
            return Err(ControlLoopEventError::RouteEventConnectionMismatch);
        }
        let admitted = connection.admitted_lines.iter().any(|candidate| {
            &candidate.line_id == line_id
                && &candidate.binding.binding_id == binding_id
                && connection.permits_line(candidate)
        });
        if !admitted {
            return Err(ControlLoopEventError::RouteOutsideSealedCandidates);
        }
        Ok(())
    }

    /// Checks an unavailable Host observation against one exact immutable
    /// Plan realization. Host, Boot, and offer generation must all be the
    /// identities sealed by that same Plan.
    pub fn validate_host_event(&self, active_plan: &Plan) -> Result<(), ControlLoopEventError> {
        self.validate()?;
        if !crate::verify_plan(active_plan) {
            return Err(ControlLoopEventError::InvalidPlan);
        }
        let Self::HostBecameUnavailable {
            plan_id,
            host_id,
            boot_id,
            offer_generation,
            ..
        } = self
        else {
            return Ok(());
        };
        if plan_id != &active_plan.plan_id {
            return Err(ControlLoopEventError::HostEventPlanMismatch);
        }
        if !active_plan.fragments.iter().any(|fragment| {
            &fragment.host_id == host_id
                && &fragment.boot_id == boot_id
                && fragment.offer_generation == *offer_generation
        }) {
            return Err(ControlLoopEventError::HostOutsideSealedPlan);
        }
        Ok(())
    }
}

fn nonempty(value: &str) -> bool {
    !value.is_empty()
}

/// Identity-only helper for presentations that need to render a selected route
/// without copying base or credential facts into the event vocabulary.
pub fn selected_admitted_line<'a>(
    event: &ControlLoopEvent,
    connection: &'a PlannedConnection,
) -> Option<&'a AdmittedLine> {
    let ControlLoopEvent::LineSelectionChanged {
        selected_line_id,
        selected_binding_id,
        ..
    } = event
    else {
        return None;
    };
    connection.admitted_lines.iter().find(|candidate| {
        &candidate.line_id == selected_line_id
            && &candidate.binding.binding_id == selected_binding_id
    })
}
