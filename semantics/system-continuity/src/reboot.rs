use alloc::string::{String, ToString};
use alloc::vec;
use conduit_core::{
    bind_sign, kind_id, ArtifactId, AuthorityContractId, AuthorityGrantId, AuthorityRequirement,
    BootId, CapabilityId, CapabilityLimits, CapabilityOffer, CheckedFace, ExecutionProfileId,
    HostAdvertisement, HostOperationContractId, HostOperationRequirement, ImplementationId,
    KindContractRevision, LineId, PortDescriptor, PortDirection, PortId, PortTemporal, SignId,
    PROTOCOL_VERSION,
};
use conduit_observatory::{HostReport, OperationalState};
use conduit_wire::SessionBinding;
use serde::{Deserialize, Serialize};

use crate::{DelegatedTransitionGrant, HostInstance};

pub const REBOOT_OPERATION: &str = "lifecycle/reboot";
pub const REBOOT_CONTRACT_REVISION: &str = "conduit.lifecycle/reboot@1";
pub const REBOOT_HOST_OPERATION: &str = "conduit.host/lifecycle-reboot@1";
pub const REBOOT_AUTHORITY_CONTRACT: &str = "conduit.authority/lifecycle-reboot@1";

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct RebootRequestId(String);

impl From<&str> for RebootRequestId {
    fn from(value: &str) -> Self {
        Self(value.to_string())
    }
}

impl RebootRequestId {
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// One optional reboot realization. Its name and revision are provenance;
/// callable compatibility is the returned canonical checked face.
pub fn delegated_reboot_offer(
    capability_id: CapabilityId,
    implementation_id: ImplementationId,
    artifact_id: ArtifactId,
) -> CapabilityOffer {
    CapabilityOffer {
        startup_parameters: vec![],
        shorthand: Some((PortId::from("request"), PortId::from("receipt"))),
        capability_id,
        kind_id: kind_id(REBOOT_OPERATION),
        kind_contract_revision: KindContractRevision::from(REBOOT_CONTRACT_REVISION),
        implementation: conduit_core::ImplementationOffer {
            execution_profile_id: ExecutionProfileId::from("lifecycle/reboot-bounded@1"),
            implementation_id,
            artifact_id,
        },
        inputs: vec![PortDescriptor {
            port_id: PortId::from("request"),
            value_kind: kind_id("lifecycle/reboot-request"),
            direction: PortDirection::Input,
            temporal: PortTemporal::Value,
        }],
        outputs: vec![PortDescriptor {
            port_id: PortId::from("receipt"),
            value_kind: kind_id("lifecycle/reboot-receipt"),
            direction: PortDirection::Output,
            temporal: PortTemporal::Value,
        }],
        host_operations: vec![HostOperationRequirement {
            contract_id: HostOperationContractId::from(REBOOT_HOST_OPERATION),
            target_kind: None,
            maximum_in_flight: 1,
            maximum_input_bytes: 0,
            maximum_output_bytes: 0,
        }],
        resource_requirements: vec![],
        authority_requirements: vec![AuthorityRequirement {
            contract_id: AuthorityContractId::from(REBOOT_AUTHORITY_CONTRACT),
            host_operation_contract_id: HostOperationContractId::from(REBOOT_HOST_OPERATION),
            subject_kind: kind_id("authority/peer"),
        }],
        limits: CapabilityLimits {
            max_active_instances: 1,
            max_queue_items: 1,
            max_queue_bytes: 0,
        },
    }
}

pub fn delegated_reboot_face() -> CheckedFace {
    delegated_reboot_offer(
        CapabilityId::from("face-only/reboot"),
        ImplementationId::from("face-only/reboot"),
        ArtifactId::from("face-only/reboot"),
    )
    .checked_face()
}

/// Reboot consumes the existing external delegated-transition authority fact;
/// it does not introduce a parallel grant vocabulary or authority store.
pub type DelegatedRebootGrant = DelegatedTransitionGrant;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RebootRequest {
    pub request_id: RebootRequestId,
    pub controller: HostInstance,
    pub target: HostInstance,
    pub required_face: CheckedFace,
    pub selected_line_id: LineId,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum RebootDenial {
    MalformedRequest,
    Unsupported,
    Unauthorized,
    StaleTargetBoot,
    SessionMismatch,
    Replay,
    AttemptLimitReached,
    TransactionPending,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RebootRejectionReceipt {
    pub request_id: RebootRequestId,
    pub target: HostInstance,
    pub reason: RebootDenial,
    pub sign_id: SignId,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RebootAcceptanceReceipt {
    pub request_id: RebootRequestId,
    pub grant_id: AuthorityGrantId,
    pub target: HostInstance,
    pub attempts_used: u16,
    pub attempts_remaining: u16,
    pub sign_id: SignId,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RebootCompletionProof {
    pub acceptance: RebootAcceptanceReceipt,
    pub old_boot_terminal_sign: SignId,
    pub new_boot: BootId,
    pub post_boot_sign: SignId,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum RebootDecision {
    Accepted(RebootAcceptanceReceipt),
    Denied(RebootRejectionReceipt),
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum RebootPendingState {
    Idle,
    Accepted,
    AwaitingReplacement,
    Completed,
    UnknownProofWindowExpired,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum RebootProgressError {
    RequestMismatch,
    NotAccepted,
    OldBootNotTerminated,
    ReplacementHostMismatch,
    ReplacementBootReused,
    ReplacementUnavailable,
    ProofWindowExpired,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum LineLossDisposition {
    IntentionalTransitionPending,
    OrdinaryTransportFailure,
}

/// Finite state for one exact delegated grant. It owns no scheduler, line,
/// membership table, authority store, or platform reboot implementation.
pub struct DelegatedRebootTransaction {
    grant: DelegatedRebootGrant,
    attempts_used: u16,
    last_request: Option<RebootRequestId>,
    acceptance: Option<RebootAcceptanceReceipt>,
    old_boot_terminal_sign: Option<SignId>,
    remaining_proof_ticks: u16,
    state: RebootPendingState,
}

impl DelegatedRebootTransaction {
    pub fn new(grant: DelegatedRebootGrant) -> Self {
        Self {
            remaining_proof_ticks: grant.proof_window_ticks,
            grant,
            attempts_used: 0,
            last_request: None,
            acceptance: None,
            old_boot_terminal_sign: None,
            state: RebootPendingState::Idle,
        }
    }

    pub const fn state(&self) -> RebootPendingState {
        self.state
    }

    pub const fn attempts_used(&self) -> u16 {
        self.attempts_used
    }

    pub fn submit(
        &mut self,
        request: &RebootRequest,
        target: &HostAdvertisement,
        session: &SessionBinding,
    ) -> RebootDecision {
        let reason = if self.last_request.as_ref() == Some(&request.request_id) {
            Some(RebootDenial::Replay)
        } else if self.state != RebootPendingState::Idle {
            Some(RebootDenial::TransactionPending)
        } else if self.attempts_used >= self.grant.maximum_transitions {
            Some(RebootDenial::AttemptLimitReached)
        } else {
            self.attempts_used = self.attempts_used.saturating_add(1);
            self.last_request = Some(request.request_id.clone());
            self.validate_request(request, target, session)
        };

        if let Some(reason) = reason {
            return RebootDecision::Denied(RebootRejectionReceipt {
                request_id: request.request_id.clone(),
                target: request.target.clone(),
                reason,
                sign_id: bind_sign(
                    &request.target.host_id,
                    &request.target.boot_id,
                    None,
                    self.grant
                        .sign_sequence_base
                        .saturating_add(u64::from(self.attempts_used) * 2 + 1),
                )
                .sign_id,
            });
        }

        let receipt = RebootAcceptanceReceipt {
            request_id: request.request_id.clone(),
            grant_id: self.grant.grant_id.clone(),
            target: request.target.clone(),
            attempts_used: self.attempts_used,
            attempts_remaining: self.grant.maximum_transitions - self.attempts_used,
            sign_id: bind_sign(
                &request.target.host_id,
                &request.target.boot_id,
                None,
                self.grant
                    .sign_sequence_base
                    .saturating_add(u64::from(self.attempts_used) * 2),
            )
            .sign_id,
        };
        self.acceptance = Some(receipt.clone());
        self.remaining_proof_ticks = self.grant.proof_window_ticks;
        self.state = RebootPendingState::Accepted;
        RebootDecision::Accepted(receipt)
    }

    fn validate_request(
        &self,
        request: &RebootRequest,
        target: &HostAdvertisement,
        session: &SessionBinding,
    ) -> Option<RebootDenial> {
        if request.request_id.as_str().is_empty()
            || self.grant.grant_id.as_str().is_empty()
            || self.grant.capability_id.as_str().is_empty()
            || self.grant.selected_line_id.as_str().is_empty()
            || self.grant.proof_window_ticks == 0
            || target.protocol_version != PROTOCOL_VERSION
        {
            return Some(RebootDenial::MalformedRequest);
        }
        if request.target.host_id != target.host_id || request.target.boot_id != target.boot_id {
            return Some(RebootDenial::StaleTargetBoot);
        }
        if request.controller != self.grant.controller || request.target != self.grant.subject {
            return Some(RebootDenial::Unauthorized);
        }
        if session.validate().is_err()
            || request.selected_line_id != session.attachment.line_id
            || request.selected_line_id != self.grant.selected_line_id
            || session.source.host_id != request.controller.host_id
            || session.source.boot_id != request.controller.boot_id
            || session.sink.host_id != request.target.host_id
            || session.sink.boot_id != request.target.boot_id
        {
            return Some(RebootDenial::SessionMismatch);
        }
        let compatible = target
            .capabilities
            .iter()
            .any(|offer| offer.checked_face() == request.required_face);
        if !compatible {
            Some(RebootDenial::Unsupported)
        } else if target
            .capabilities
            .iter()
            .filter(|offer| offer.checked_face() == request.required_face)
            .all(|offer| offer.capability_id != self.grant.capability_id)
        {
            Some(RebootDenial::Unauthorized)
        } else {
            None
        }
    }

    pub fn control_line_lost(&self) -> LineLossDisposition {
        match self.state {
            RebootPendingState::Accepted | RebootPendingState::AwaitingReplacement => {
                LineLossDisposition::IntentionalTransitionPending
            }
            _ => LineLossDisposition::OrdinaryTransportFailure,
        }
    }

    pub fn old_boot_terminated(
        &mut self,
        request_id: &RebootRequestId,
        sign_id: SignId,
    ) -> Result<(), RebootProgressError> {
        let acceptance = self
            .acceptance
            .as_ref()
            .ok_or(RebootProgressError::NotAccepted)?;
        if &acceptance.request_id != request_id {
            return Err(RebootProgressError::RequestMismatch);
        }
        if self.state == RebootPendingState::UnknownProofWindowExpired {
            return Err(RebootProgressError::ProofWindowExpired);
        }
        if self.state != RebootPendingState::Accepted {
            return Err(RebootProgressError::NotAccepted);
        }
        self.old_boot_terminal_sign = Some(sign_id);
        self.state = RebootPendingState::AwaitingReplacement;
        Ok(())
    }

    pub fn tick_proof_window(&mut self) {
        if !matches!(
            self.state,
            RebootPendingState::Accepted | RebootPendingState::AwaitingReplacement
        ) {
            return;
        }
        self.remaining_proof_ticks = self.remaining_proof_ticks.saturating_sub(1);
        if self.remaining_proof_ticks == 0 {
            self.state = RebootPendingState::UnknownProofWindowExpired;
        }
    }

    pub fn observe_replacement(
        &mut self,
        request_id: &RebootRequestId,
        report: &HostReport,
        post_boot_sign: SignId,
    ) -> Result<RebootCompletionProof, RebootProgressError> {
        if self.state == RebootPendingState::UnknownProofWindowExpired {
            return Err(RebootProgressError::ProofWindowExpired);
        }
        if self.state != RebootPendingState::AwaitingReplacement {
            return Err(RebootProgressError::OldBootNotTerminated);
        }
        let acceptance = self
            .acceptance
            .as_ref()
            .ok_or(RebootProgressError::NotAccepted)?;
        if &acceptance.request_id != request_id {
            return Err(RebootProgressError::RequestMismatch);
        }
        if report.advertisement.host_id != acceptance.target.host_id {
            return Err(RebootProgressError::ReplacementHostMismatch);
        }
        if report.advertisement.boot_id == acceptance.target.boot_id {
            return Err(RebootProgressError::ReplacementBootReused);
        }
        if report.state != OperationalState::Available {
            return Err(RebootProgressError::ReplacementUnavailable);
        }
        let proof = RebootCompletionProof {
            acceptance: acceptance.clone(),
            old_boot_terminal_sign: self
                .old_boot_terminal_sign
                .clone()
                .ok_or(RebootProgressError::OldBootNotTerminated)?,
            new_boot: report.advertisement.boot_id.clone(),
            post_boot_sign,
        };
        self.state = RebootPendingState::Completed;
        Ok(proof)
    }
}
