//! Typed new-Plan recovery inside one Body Wake.

use alloc::vec::Vec;

use conduit_body::{Body, BodyId, BodyLifecycleError, Wake, WakeId};
use conduit_core::{
    bind_active_play, ActivePlayId, ActivePlayIdentity, BootId, ClueId, ControlLoopEvent, GearId,
    HostId, LinkAvailability, LinkObservation, Plan, PlanId, PlanningRequestAuthority,
    PlayUnsatisfiedReason,
};
use conduit_wire::{RouteDisposition, RouteError, RouteMachine, SessionBinding, WireError};
use serde::{Deserialize, Serialize};

pub const MAX_R1_RECOVERY_EVENTS: usize = 8;
pub const MAX_R1_LED_RESULT_CLUES: usize = 4;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct R1LedResultClue {
    pub body_id: BodyId,
    pub wake_id: WakeId,
    pub plan_id: PlanId,
    pub active_play_id: ActivePlayId,
    pub pico_host_id: HostId,
    pub pico_boot_id: BootId,
    pub clue_id: ClueId,
    pub level: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct R1LedResultObservation {
    pub pico_host_id: HostId,
    pub pico_boot_id: BootId,
    pub plan_id: PlanId,
    pub active_play_id: ActivePlayId,
    pub observed_session: SessionBinding,
    pub clue_id: ClueId,
    pub level: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct R1RecoveryStartClues {
    pub birth: ClueId,
    pub wake: ClueId,
    pub plan_ready: ClueId,
    pub play_started: ClueId,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct R1ReplacementClues {
    pub request: ClueId,
    pub planned: ClueId,
    pub superseded: ClueId,
    pub realized: ClueId,
    pub play_started: ClueId,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct R1NewPlanRecovery {
    body: Body,
    wake: Wake,
    obligation_gear_id: GearId,
    plan_a: Plan,
    play_a: ActivePlayIdentity,
    plan_b: Option<Plan>,
    play_b: Option<ActivePlayIdentity>,
    events: Vec<ControlLoopEvent>,
    led_results: Vec<R1LedResultClue>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum R1RecoveryError {
    Lifecycle(BodyLifecycleError),
    Route(RouteError),
    Wire(WireError),
    InvalidPlan,
    InvalidObservation,
    NotUnsatisfied,
    WrongRealizationSubject,
    StaleResult,
    CapacityExhausted,
}

impl From<BodyLifecycleError> for R1RecoveryError {
    fn from(value: BodyLifecycleError) -> Self {
        Self::Lifecycle(value)
    }
}

impl From<RouteError> for R1RecoveryError {
    fn from(value: RouteError) -> Self {
        Self::Route(value)
    }
}

impl From<WireError> for R1RecoveryError {
    fn from(value: WireError) -> Self {
        Self::Wire(value)
    }
}

impl R1NewPlanRecovery {
    #[allow(clippy::too_many_arguments)]
    pub fn begin(
        plan_a: Plan,
        obligation_gear_id: GearId,
        birth_sequence: u64,
        wake_sequence: u64,
        play_host_id: HostId,
        play_boot_id: BootId,
        play_sequence: u64,
        clues: R1RecoveryStartClues,
    ) -> Result<Self, R1RecoveryError> {
        if !conduit_core::verify_plan(&plan_a) || find_gear(&plan_a, &obligation_gear_id).is_none()
        {
            return Err(R1RecoveryError::InvalidPlan);
        }
        let body = Body::born(
            plan_a.source_document_id.clone(),
            plan_a.checked_form_id.clone(),
            birth_sequence,
            clues.birth,
        )?;
        let (body, wake) = body.wake(wake_sequence, clues.wake)?;
        let wake = wake.plan_ready(&plan_a, clues.plan_ready)?;
        let play_a = bind_active_play(&plan_a.plan_id, &play_host_id, &play_boot_id, play_sequence);
        let wake = wake.play_started(&play_a, clues.play_started)?;
        Ok(Self {
            body,
            wake,
            obligation_gear_id,
            plan_a,
            play_a,
            plan_b: None,
            play_b: None,
            events: Vec::with_capacity(MAX_R1_RECOVERY_EVENTS),
            led_results: Vec::with_capacity(MAX_R1_LED_RESULT_CLUES),
        })
    }

    pub fn observe_route_unavailable(
        &mut self,
        observation: LinkObservation,
        unsatisfied_clue_id: ClueId,
    ) -> Result<(), R1RecoveryError> {
        if observation.availability != LinkAvailability::Unavailable || self.plan_b.is_some() {
            return Err(R1RecoveryError::InvalidObservation);
        }
        let connection = self
            .plan_a
            .fragments
            .iter()
            .flat_map(|fragment| &fragment.connections)
            .find(|connection| {
                connection
                    .route_candidates
                    .iter()
                    .any(|route| route.binding_id == observation.binding_id)
            })
            .ok_or(R1RecoveryError::InvalidObservation)?;
        let mut routes = RouteMachine::new(connection)?;
        let update = routes.observe(&observation)?;
        if !matches!(
            update.disposition,
            RouteDisposition::Unsatisfied {
                replan_may_be_requested: true
            }
        ) {
            return Err(R1RecoveryError::NotUnsatisfied);
        }
        let unavailable = ControlLoopEvent::LinkBecameUnavailable {
            plan_id: self.plan_a.plan_id.clone(),
            connection_id: connection.connection_id.clone(),
            binding_id: observation.binding_id,
            observation_clue_id: observation.clue_id,
        };
        unavailable
            .validate_route_event(&self.plan_a.plan_id, connection)
            .map_err(|_| R1RecoveryError::InvalidObservation)?;
        let unsatisfied = ControlLoopEvent::PlayBecameUnsatisfied {
            plan_id: self.plan_a.plan_id.clone(),
            reason: PlayUnsatisfiedReason::NoAdmittedRouteReady,
            clue_id: unsatisfied_clue_id.clone(),
        };
        unsatisfied
            .validate()
            .map_err(|_| R1RecoveryError::InvalidObservation)?;
        self.reserve_event_slots(2)?;
        let next_wake = self
            .wake
            .became_unsatisfied(&self.plan_a.plan_id, unsatisfied_clue_id)?;
        self.push_event(unavailable)?;
        self.push_event(unsatisfied)?;
        self.wake = next_wake;
        Ok(())
    }

    #[allow(clippy::too_many_arguments)]
    pub fn install_replacement(
        &mut self,
        plan_b: Plan,
        requester_host_id: HostId,
        requester_boot_id: BootId,
        play_host_id: HostId,
        play_boot_id: BootId,
        play_sequence: u64,
        clues: R1ReplacementClues,
    ) -> Result<(), R1RecoveryError> {
        if self.plan_b.is_some()
            || self.wake.lifecycle != conduit_body::WakeLifecycle::Unsatisfied
            || !conduit_core::verify_plan(&plan_b)
            || plan_b.plan_id == self.plan_a.plan_id
        {
            return Err(R1RecoveryError::InvalidPlan);
        }
        let prior_gear = find_gear(&self.plan_a, &self.obligation_gear_id)
            .ok_or(R1RecoveryError::InvalidPlan)?;
        let replacement_gear = find_gear(&plan_b, &self.obligation_gear_id)
            .ok_or(R1RecoveryError::WrongRealizationSubject)?;
        if prior_gear.host_id != replacement_gear.host_id
            || prior_gear.boot_id != replacement_gear.boot_id
            || prior_gear.capability_id != replacement_gear.capability_id
            || prior_gear.kind_id != replacement_gear.kind_id
            || prior_gear.inputs != replacement_gear.inputs
            || prior_gear.outputs != replacement_gear.outputs
        {
            return Err(R1RecoveryError::WrongRealizationSubject);
        }
        let request = ControlLoopEvent::PlanningRequested {
            prior_plan_id: self.plan_a.plan_id.clone(),
            requester_host_id,
            requester_boot_id,
            authority: PlanningRequestAuthority::HostLocal,
            request_clue_id: clues.request.clone(),
        };
        let wake_replan_clue = clues.planned.clone();
        let planned = ControlLoopEvent::PlanningSucceeded {
            prior_plan_id: self.plan_a.plan_id.clone(),
            replacement_plan_id: plan_b.plan_id.clone(),
            request_clue_id: clues.request,
            clue_id: clues.planned,
        };
        let superseded = ControlLoopEvent::PlanSuperseded {
            prior_plan_id: self.plan_a.plan_id.clone(),
            replacement_plan_id: plan_b.plan_id.clone(),
            clue_id: clues.superseded,
        };
        let realized = ControlLoopEvent::PlanRealized {
            plan_id: plan_b.plan_id.clone(),
            clue_id: clues.realized,
        };
        for event in [&request, &planned, &superseded, &realized] {
            event.validate().map_err(|_| R1RecoveryError::InvalidPlan)?;
        }
        self.reserve_event_slots(4)?;
        let next_wake = self.wake.plan_ready(&plan_b, wake_replan_clue)?;
        self.push_event(request)?;
        self.push_event(planned)?;
        self.push_event(superseded)?;
        self.push_event(realized)?;
        let play_b = bind_active_play(&plan_b.plan_id, &play_host_id, &play_boot_id, play_sequence);
        self.wake = next_wake.play_started(&play_b, clues.play_started)?;
        self.plan_b = Some(plan_b);
        self.play_b = Some(play_b);
        Ok(())
    }

    pub fn refuse_replacement(
        &mut self,
        requester_host_id: HostId,
        requester_boot_id: BootId,
        request_clue_id: ClueId,
        refusal_clue_id: ClueId,
        reason: conduit_core::PlanningRefusalReason,
    ) -> Result<(), R1RecoveryError> {
        if self.plan_b.is_some() || self.wake.lifecycle != conduit_body::WakeLifecycle::Unsatisfied
        {
            return Err(R1RecoveryError::InvalidPlan);
        }
        let request = ControlLoopEvent::PlanningRequested {
            prior_plan_id: self.plan_a.plan_id.clone(),
            requester_host_id,
            requester_boot_id,
            authority: PlanningRequestAuthority::HostLocal,
            request_clue_id: request_clue_id.clone(),
        };
        let refusal = ControlLoopEvent::PlanningRefused {
            prior_plan_id: self.plan_a.plan_id.clone(),
            request_clue_id,
            reason,
            clue_id: refusal_clue_id,
        };
        for event in [&request, &refusal] {
            event.validate().map_err(|_| R1RecoveryError::InvalidPlan)?;
        }
        self.reserve_event_slots(2)?;
        self.push_event(request)?;
        self.push_event(refusal)
    }

    pub fn record_led_result(
        &mut self,
        observation: R1LedResultObservation,
    ) -> Result<(), R1RecoveryError> {
        let (current_plan, current_play) = self
            .plan_b
            .as_ref()
            .zip(self.play_b.as_ref())
            .ok_or(R1RecoveryError::StaleResult)?;
        if observation.plan_id != current_plan.plan_id
            || observation.active_play_id != current_play.active_play_id
        {
            return Err(R1RecoveryError::StaleResult);
        }
        let planned_session = session_binding(current_plan, &self.obligation_gear_id)?;
        let expected_session = planned_session.with_observed_boots(
            observation.observed_session.source.boot_id.clone(),
            observation.observed_session.sink.boot_id.clone(),
        )?;
        if observation.observed_session != expected_session
            || observation.pico_host_id != observation.observed_session.sink.host_id
            || observation.pico_boot_id != observation.observed_session.sink.boot_id
        {
            return Err(R1RecoveryError::StaleResult);
        }
        if self.led_results.len() >= MAX_R1_LED_RESULT_CLUES {
            return Err(R1RecoveryError::CapacityExhausted);
        }
        self.led_results.push(R1LedResultClue {
            body_id: self.body.body_id.clone(),
            wake_id: self.wake.wake_id.clone(),
            plan_id: observation.plan_id,
            active_play_id: observation.active_play_id,
            pico_host_id: observation.pico_host_id,
            pico_boot_id: observation.pico_boot_id,
            clue_id: observation.clue_id,
            level: observation.level,
        });
        Ok(())
    }

    pub fn body(&self) -> &Body {
        &self.body
    }

    pub fn wake(&self) -> &Wake {
        &self.wake
    }

    pub fn plan_a(&self) -> &Plan {
        &self.plan_a
    }

    pub fn play_a(&self) -> &ActivePlayIdentity {
        &self.play_a
    }

    pub fn plan_b(&self) -> Option<&Plan> {
        self.plan_b.as_ref()
    }

    pub fn play_b(&self) -> Option<&ActivePlayIdentity> {
        self.play_b.as_ref()
    }

    pub fn plan_a_session_binding(&self) -> Result<SessionBinding, R1RecoveryError> {
        session_binding(&self.plan_a, &self.obligation_gear_id)
    }

    pub fn plan_b_session_binding(&self) -> Result<SessionBinding, R1RecoveryError> {
        session_binding(
            self.plan_b.as_ref().ok_or(R1RecoveryError::InvalidPlan)?,
            &self.obligation_gear_id,
        )
    }

    pub fn events(&self) -> &[ControlLoopEvent] {
        &self.events
    }

    pub fn led_results(&self) -> &[R1LedResultClue] {
        &self.led_results
    }

    fn push_event(&mut self, event: ControlLoopEvent) -> Result<(), R1RecoveryError> {
        if self.events.len() >= MAX_R1_RECOVERY_EVENTS {
            return Err(R1RecoveryError::CapacityExhausted);
        }
        self.events.push(event);
        Ok(())
    }

    fn reserve_event_slots(&self, count: usize) -> Result<(), R1RecoveryError> {
        if self.events.len().saturating_add(count) > MAX_R1_RECOVERY_EVENTS {
            Err(R1RecoveryError::CapacityExhausted)
        } else {
            Ok(())
        }
    }
}

fn find_gear<'a>(plan: &'a Plan, gear_id: &GearId) -> Option<&'a conduit_core::PlannedGear> {
    plan.fragments
        .iter()
        .flat_map(|fragment| &fragment.placements)
        .find(|gear| &gear.gear_id == gear_id)
}

fn session_binding(
    plan: &Plan,
    obligation_gear_id: &GearId,
) -> Result<SessionBinding, R1RecoveryError> {
    let sink = find_gear(plan, obligation_gear_id).ok_or(R1RecoveryError::InvalidPlan)?;
    let connection = plan
        .fragments
        .iter()
        .flat_map(|fragment| &fragment.connections)
        .find(|connection| connection.sink_placement_id == sink.placement_id)
        .ok_or(R1RecoveryError::InvalidPlan)?;
    let source_fragment = plan
        .fragments
        .iter()
        .find(|fragment| {
            fragment
                .placements
                .iter()
                .any(|gear| gear.placement_id == connection.source_placement_id)
        })
        .ok_or(R1RecoveryError::InvalidPlan)?;
    let sink_fragment = plan
        .fragments
        .iter()
        .find(|fragment| {
            fragment.fragment_id != source_fragment.fragment_id && fragment.host_id == sink.host_id
        })
        .ok_or(R1RecoveryError::InvalidPlan)?;
    SessionBinding::from_planned_connection(
        plan.plan_id.clone(),
        source_fragment.fragment_id.clone(),
        sink_fragment.fragment_id.clone(),
        connection,
    )
    .map_err(Into::into)
}
