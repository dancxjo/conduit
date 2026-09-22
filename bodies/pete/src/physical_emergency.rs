//! Pete's exact Pico W physical emergency binding and receipt admission.

use conduit_body::{
    BodyId, EmergencyControl, EmergencyMachineAction, EmergencyOutcome, EmergencyPolicy,
    EmergencyRefusal, EmergencyRequest, EmergencyStepOutcome, EmergencyTriggerClass,
    EMERGENCY_CONTROL_POLICY,
};
use conduit_core::{BootId, HostId};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

pub const PETE_PHYSICAL_EMERGENCY_SCHEMA: &str = "conduit.pete/physical-emergency-receipt@1";
pub const PETE_PHYSICAL_EMERGENCY_PROVIDER: &str = "rp2040/gpio22-active-low@1";
pub const PETE_PHYSICAL_EMERGENCY_PIN: u8 = 22;
const ARM_PREFIX: &str = "CONDUIT_PHYSICAL_EMERGENCY_ARM@1";

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum PetePhysicalEmergencyRefusal {
    InvalidBuild,
    UnsupportedPolicy,
    InvalidReceipt,
    WrongBinding,
    NotTriggered,
    IncompleteLocalReduction,
    Emergency(EmergencyRefusal),
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct PetePhysicalEmergencyReceipt {
    pub schema: String,
    pub build_id: String,
    pub binding_sha256: String,
    pub provider_id: String,
    pub gpio: u8,
    pub active_low: bool,
    pub triggered: bool,
    pub observed_at_millis: u64,
    pub translator_disabled: bool,
    pub play_preempted: bool,
}

/// Host-side admission for the dedicated physical switch. The firmware gets
/// only a digest of current truth and a deliberately narrow reduction policy;
/// it never receives general Body authority.
pub struct PetePhysicalEmergencyAdapter {
    build_id: String,
    binding_sha256: String,
    body_id: BodyId,
    host_id: HostId,
    boot_id: BootId,
    control: EmergencyControl,
}

impl PetePhysicalEmergencyAdapter {
    pub fn admit(
        build_id: impl Into<String>,
        body_id: BodyId,
        host_id: HostId,
        boot_id: BootId,
        policy: EmergencyPolicy,
    ) -> Result<Self, PetePhysicalEmergencyRefusal> {
        let build_id = build_id.into();
        if build_id.is_empty() || build_id.len() > 192 || build_id.chars().any(char::is_whitespace)
        {
            return Err(PetePhysicalEmergencyRefusal::InvalidBuild);
        }
        // This adapter owns one exact local reduction: revoke Pete's Create
        // actuation boundary. Lull, carriers, propagation, halt and reset need
        // their own owners and must not be implied by this switch.
        if !policy.allow_physical
            || !policy.revoke_local_execution
            || policy.attempt_graceful_lull
            || policy.isolate_carriers
            || policy.terminal_action != EmergencyMachineAction::None
        {
            return Err(PetePhysicalEmergencyRefusal::UnsupportedPolicy);
        }
        let binding_sha256 = binding_digest(&build_id, &body_id, &host_id, &boot_id, policy);
        Ok(Self {
            build_id,
            binding_sha256,
            body_id: body_id.clone(),
            host_id: host_id.clone(),
            boot_id: boot_id.clone(),
            control: EmergencyControl::admit(body_id, host_id, boot_id, policy),
        })
    }

    pub fn arm_command(&self) -> String {
        format!("{ARM_PREFIX}:{}:{}", self.build_id, self.binding_sha256)
    }

    pub fn binding_sha256(&self) -> &str {
        &self.binding_sha256
    }

    pub fn inspect(
        &mut self,
        receipt: &PetePhysicalEmergencyReceipt,
    ) -> Result<EmergencyOutcome, PetePhysicalEmergencyRefusal> {
        if receipt.schema != PETE_PHYSICAL_EMERGENCY_SCHEMA
            || receipt.build_id != self.build_id
            || receipt.provider_id != PETE_PHYSICAL_EMERGENCY_PROVIDER
            || receipt.gpio != PETE_PHYSICAL_EMERGENCY_PIN
            || !receipt.active_low
            || receipt.observed_at_millis == 0
        {
            return Err(PetePhysicalEmergencyRefusal::InvalidReceipt);
        }
        if receipt.binding_sha256 != self.binding_sha256 {
            return Err(PetePhysicalEmergencyRefusal::WrongBinding);
        }
        if !receipt.triggered {
            return Err(PetePhysicalEmergencyRefusal::NotTriggered);
        }
        if !receipt.translator_disabled || !receipt.play_preempted {
            return Err(PetePhysicalEmergencyRefusal::IncompleteLocalReduction);
        }
        let freshness = receipt
            .observed_at_millis
            .checked_add(1)
            .ok_or(PetePhysicalEmergencyRefusal::InvalidReceipt)?;
        let mut outcome = self
            .control
            .inspect(&EmergencyRequest {
                request_id: format!("pete/physical-emergency/{freshness}"),
                body_id: self.body_id.clone(),
                host_id: self.host_id.clone(),
                boot_id: self.boot_id.clone(),
                trigger: EmergencyTriggerClass::LocalPhysicalEmergency,
                policy_id: EMERGENCY_CONTROL_POLICY.into(),
                freshness,
            })
            .map_err(PetePhysicalEmergencyRefusal::Emergency)?;
        outcome.local_execution_revocation = EmergencyStepOutcome::Completed;
        Ok(outcome)
    }
}

fn binding_digest(
    build_id: &str,
    body_id: &BodyId,
    host_id: &HostId,
    boot_id: &BootId,
    policy: EmergencyPolicy,
) -> String {
    let mut hash = Sha256::new();
    for value in [
        PETE_PHYSICAL_EMERGENCY_SCHEMA,
        PETE_PHYSICAL_EMERGENCY_PROVIDER,
        build_id,
        body_id.as_str(),
        host_id.as_str(),
        boot_id.as_str(),
        EMERGENCY_CONTROL_POLICY,
    ] {
        hash.update((value.len() as u64).to_le_bytes());
        hash.update(value.as_bytes());
    }
    hash.update([
        policy.allow_keyboard_rescue as u8,
        policy.allow_physical as u8,
        policy.allow_acoustic as u8,
        policy.allow_remote as u8,
        policy.attempt_graceful_lull as u8,
        policy.revoke_local_execution as u8,
        policy.isolate_carriers as u8,
        policy.terminal_action as u8,
        PETE_PHYSICAL_EMERGENCY_PIN,
    ]);
    format!("{:x}", hash.finalize())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn policy() -> EmergencyPolicy {
        EmergencyPolicy {
            allow_keyboard_rescue: false,
            allow_physical: true,
            allow_acoustic: false,
            allow_remote: false,
            attempt_graceful_lull: false,
            revoke_local_execution: true,
            isolate_carriers: false,
            terminal_action: EmergencyMachineAction::None,
        }
    }

    fn body_id() -> BodyId {
        serde_json::from_str("\"body/pete\"").unwrap()
    }

    fn adapter() -> PetePhysicalEmergencyAdapter {
        PetePhysicalEmergencyAdapter::admit(
            "pete-build-1",
            body_id(),
            HostId::from("host/pete-pico"),
            BootId::from("boot/pete-pico/1"),
            policy(),
        )
        .unwrap()
    }

    fn receipt(adapter: &PetePhysicalEmergencyAdapter) -> PetePhysicalEmergencyReceipt {
        PetePhysicalEmergencyReceipt {
            schema: PETE_PHYSICAL_EMERGENCY_SCHEMA.into(),
            build_id: "pete-build-1".into(),
            binding_sha256: adapter.binding_sha256().into(),
            provider_id: PETE_PHYSICAL_EMERGENCY_PROVIDER.into(),
            gpio: PETE_PHYSICAL_EMERGENCY_PIN,
            active_low: true,
            triggered: true,
            observed_at_millis: 41,
            translator_disabled: true,
            play_preempted: true,
        }
    }

    #[test]
    fn exact_physical_receipt_enters_the_shared_one_shot_path() {
        let mut adapter = adapter();
        assert!(adapter.arm_command().ends_with(adapter.binding_sha256()));
        let value = adapter.inspect(&receipt(&adapter)).unwrap();
        assert_eq!(value.trigger, EmergencyTriggerClass::LocalPhysicalEmergency);
        assert_eq!(
            value.local_execution_revocation,
            EmergencyStepOutcome::Completed
        );
        assert_eq!(value.graceful_lull, EmergencyStepOutcome::Unavailable);
        assert_eq!(value.carrier_isolation, EmergencyStepOutcome::Unavailable);
        assert!(!value.machine_action_requested);
        let replay = receipt(&adapter);
        assert!(matches!(
            adapter.inspect(&replay),
            Err(PetePhysicalEmergencyRefusal::Emergency(
                EmergencyRefusal::AlreadyTriggered
            ))
        ));
    }

    #[test]
    fn stale_binding_and_incomplete_local_reduction_refuse() {
        let mut adapter = adapter();
        let mut stale = receipt(&adapter);
        stale.binding_sha256.replace_range(..1, "x");
        assert_eq!(
            adapter.inspect(&stale),
            Err(PetePhysicalEmergencyRefusal::WrongBinding)
        );
        let mut incomplete = receipt(&adapter);
        incomplete.translator_disabled = false;
        assert_eq!(
            adapter.inspect(&incomplete),
            Err(PetePhysicalEmergencyRefusal::IncompleteLocalReduction)
        );
    }

    #[test]
    fn broader_or_terminal_policy_cannot_be_hidden_in_the_gpio_adapter() {
        for unsupported in [
            EmergencyPolicy {
                attempt_graceful_lull: true,
                ..policy()
            },
            EmergencyPolicy {
                isolate_carriers: true,
                ..policy()
            },
            EmergencyPolicy {
                terminal_action: EmergencyMachineAction::Reset,
                ..policy()
            },
        ] {
            assert!(matches!(
                PetePhysicalEmergencyAdapter::admit(
                    "pete-build-1",
                    body_id(),
                    HostId::from("host/pete-pico"),
                    BootId::from("boot/pete-pico/1"),
                    unsupported,
                ),
                Err(PetePhysicalEmergencyRefusal::UnsupportedPolicy)
            ));
        }
    }
}
