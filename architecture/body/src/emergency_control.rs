//! Bounded authority-reduction requests below ordinary form interpretation.

use alloc::string::String;
use conduit_core::{BootId, HostId};
use serde::{Deserialize, Serialize};

use crate::BodyId;

pub const EMERGENCY_CONTROL_POLICY: &str = "conduit.body/emergency-control@1";
pub const MAX_EMERGENCY_ID_BYTES: usize = 96;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum EmergencyTriggerClass {
    LocalKeyboardRescue,
    LocalPhysicalEmergency,
    LocalAcousticEmergency,
    AuthenticatedRemoteEmergency,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EmergencyRequest {
    pub request_id: String,
    pub body_id: BodyId,
    pub host_id: HostId,
    pub boot_id: BootId,
    pub trigger: EmergencyTriggerClass,
    pub policy_id: String,
    /// Monotonic within one trigger authority. Local sources may use a
    /// boot-scoped counter; remote sources must use their authenticated session.
    pub freshness: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct EmergencyPolicy {
    pub allow_keyboard_rescue: bool,
    pub allow_physical: bool,
    pub allow_acoustic: bool,
    pub allow_remote: bool,
    pub attempt_graceful_lull: bool,
    pub revoke_local_execution: bool,
    pub isolate_carriers: bool,
    pub terminal_action: EmergencyMachineAction,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum EmergencyMachineAction {
    None,
    Halt,
    Reset,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum EmergencyStepOutcome {
    NotRequested,
    Requested,
    Completed,
    Failed,
    Unavailable,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EmergencyOutcome {
    pub request_id: String,
    pub trigger: EmergencyTriggerClass,
    pub policy_id: String,
    pub graceful_lull: EmergencyStepOutcome,
    pub local_execution_revocation: EmergencyStepOutcome,
    pub carrier_isolation: EmergencyStepOutcome,
    pub propagation: EmergencyStepOutcome,
    pub machine_action: EmergencyMachineAction,
    pub machine_action_requested: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EmergencyRefusal {
    InvalidRequest,
    WrongBody,
    WrongHost,
    StaleBoot,
    TriggerDisabled,
    StaleOrReplayed,
    AlreadyTriggered,
}

/// Tiny one-shot admission boundary. Effect execution is supplied by the host;
/// this type cannot Wake, grant authority, change policy, or resume execution.
pub struct EmergencyControl {
    body_id: BodyId,
    host_id: HostId,
    boot_id: BootId,
    policy: EmergencyPolicy,
    last_freshness: [u64; 4],
    triggered: bool,
}

impl EmergencyControl {
    pub fn admit(
        body_id: BodyId,
        host_id: HostId,
        boot_id: BootId,
        policy: EmergencyPolicy,
    ) -> Self {
        Self {
            body_id,
            host_id,
            boot_id,
            policy,
            last_freshness: [0; 4],
            triggered: false,
        }
    }

    pub const fn triggered(&self) -> bool {
        self.triggered
    }

    pub fn inspect(
        &mut self,
        request: &EmergencyRequest,
    ) -> Result<EmergencyOutcome, EmergencyRefusal> {
        if self.triggered {
            return Err(EmergencyRefusal::AlreadyTriggered);
        }
        if request.request_id.is_empty()
            || request.request_id.len() > MAX_EMERGENCY_ID_BYTES
            || request.policy_id != EMERGENCY_CONTROL_POLICY
            || request.freshness == 0
        {
            return Err(EmergencyRefusal::InvalidRequest);
        }
        if request.body_id != self.body_id {
            return Err(EmergencyRefusal::WrongBody);
        }
        if request.host_id != self.host_id {
            return Err(EmergencyRefusal::WrongHost);
        }
        if request.boot_id != self.boot_id {
            return Err(EmergencyRefusal::StaleBoot);
        }
        if !self.trigger_enabled(request.trigger) {
            return Err(EmergencyRefusal::TriggerDisabled);
        }
        let index = trigger_index(request.trigger);
        if request.freshness <= self.last_freshness[index] {
            return Err(EmergencyRefusal::StaleOrReplayed);
        }
        self.last_freshness[index] = request.freshness;
        self.triggered = true;
        Ok(EmergencyOutcome {
            request_id: request.request_id.clone(),
            trigger: request.trigger,
            policy_id: request.policy_id.clone(),
            graceful_lull: requested(self.policy.attempt_graceful_lull),
            local_execution_revocation: requested(self.policy.revoke_local_execution),
            carrier_isolation: requested(self.policy.isolate_carriers),
            propagation: EmergencyStepOutcome::NotRequested,
            machine_action: self.policy.terminal_action,
            machine_action_requested: self.policy.terminal_action != EmergencyMachineAction::None,
        })
    }

    fn trigger_enabled(&self, trigger: EmergencyTriggerClass) -> bool {
        match trigger {
            EmergencyTriggerClass::LocalKeyboardRescue => self.policy.allow_keyboard_rescue,
            EmergencyTriggerClass::LocalPhysicalEmergency => self.policy.allow_physical,
            EmergencyTriggerClass::LocalAcousticEmergency => self.policy.allow_acoustic,
            EmergencyTriggerClass::AuthenticatedRemoteEmergency => self.policy.allow_remote,
        }
    }
}

const fn trigger_index(trigger: EmergencyTriggerClass) -> usize {
    match trigger {
        EmergencyTriggerClass::LocalKeyboardRescue => 0,
        EmergencyTriggerClass::LocalPhysicalEmergency => 1,
        EmergencyTriggerClass::LocalAcousticEmergency => 2,
        EmergencyTriggerClass::AuthenticatedRemoteEmergency => 3,
    }
}

const fn requested(value: bool) -> EmergencyStepOutcome {
    if value {
        EmergencyStepOutcome::Requested
    } else {
        EmergencyStepOutcome::Unavailable
    }
}
