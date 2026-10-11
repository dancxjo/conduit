//! Retained acknowledged Play cancellation; no external-operation causality.
use super::ProtocolBodyRefusal;
use crate::{
    protocol_artifact::ProtocolArtifactIdentity, protocol_host_calls::ProtocolCallRefusal,
};
use conduit_body::BodyPlayIdentity;
use conduit_composite::AdmittedKernelCompositeHostRequest;
use conduit_kernel::{HostCallId, NodeId, RequestId};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PendingRetirementCall {
    token: AdmittedKernelCompositeHostRequest,
    obligation: [u8; 32],
    node: NodeId,
    call: HostCallId,
    request: RequestId,
}
impl PendingRetirementCall {
    pub(crate) fn capture(
        token: AdmittedKernelCompositeHostRequest,
        obligation: [u8; 32],
        node: NodeId,
        call: HostCallId,
        request: RequestId,
    ) -> Self {
        Self {
            token,
            obligation,
            node,
            call,
            request,
        }
    }
    pub fn token(&self) -> &AdmittedKernelCompositeHostRequest {
        &self.token
    }
    pub fn obligation(&self) -> &[u8; 32] {
        &self.obligation
    }
    pub fn node(&self) -> NodeId {
        self.node
    }
    pub fn call(&self) -> HostCallId {
        self.call
    }
    pub fn request(&self) -> RequestId {
        self.request
    }
}

/// This value is minted only after actual owner and kernel cancellation and Lull.
/// No completed-call history or external input association is asserted.
#[derive(Debug)]
pub struct AcknowledgedProtocolCancellation {
    artifact: ProtocolArtifactIdentity,
    play: BodyPlayIdentity,
    pending: [Option<PendingRetirementCall>; 2],
}
impl AcknowledgedProtocolCancellation {
    pub(super) fn new(
        artifact: ProtocolArtifactIdentity,
        play: BodyPlayIdentity,
        pending: [Option<PendingRetirementCall>; 2],
    ) -> Self {
        Self {
            artifact,
            play,
            pending,
        }
    }
    pub fn artifact(&self) -> &ProtocolArtifactIdentity {
        &self.artifact
    }
    pub fn play(&self) -> &BodyPlayIdentity {
        &self.play
    }
    pub fn pending(&self) -> &[Option<PendingRetirementCall>; 2] {
        &self.pending
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CancellationStage {
    Identity,
    /// Includes hardware-stop or lease-release failure. Original error remains
    /// authoritative; this stage does not assert whether hardware stopped.
    NativeOwnerRetirement,
    ClockRetirement,
    ComputationRetirement,
    KernelCancellation,
    BodyLull,
}
#[derive(Debug)]
pub struct ProtocolCancellationRefusal {
    pub stage: CancellationStage,
    pub reason: ProtocolBodyRefusal,
}
impl ProtocolCancellationRefusal {
    pub(super) fn new(stage: CancellationStage, reason: ProtocolBodyRefusal) -> Self {
        Self { stage, reason }
    }
    pub(super) fn call(error: ProtocolCallRefusal) -> Self {
        let stage = match &error {
            ProtocolCallRefusal::Kernel(_) => CancellationStage::KernelCancellation,
            ProtocolCallRefusal::Clock(_) => CancellationStage::ClockRetirement,
            ProtocolCallRefusal::Expression(_) | ProtocolCallRefusal::Selector(_) => {
                CancellationStage::ComputationRetirement
            }
            _ => CancellationStage::NativeOwnerRetirement,
        };
        Self::new(stage, ProtocolBodyRefusal::Call(error))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn failed_native_retirement_preserves_refusal_without_success_evidence() {
        let refused = ProtocolCancellationRefusal::call(ProtocolCallRefusal::Unsupported);
        assert_eq!(refused.stage, CancellationStage::NativeOwnerRetirement);
        assert!(matches!(
            refused.reason,
            ProtocolBodyRefusal::Call(ProtocolCallRefusal::Unsupported)
        ));
    }

    #[test]
    fn kernel_and_lull_failure_are_distinct_from_native_retirement() {
        let kernel = ProtocolCancellationRefusal::call(ProtocolCallRefusal::Kernel(
            conduit_composite::KernelCompositeError::CancellationRefused { failed_children: 1 },
        ));
        assert_eq!(kernel.stage, CancellationStage::KernelCancellation);
        let lull = ProtocolCancellationRefusal::new(
            CancellationStage::BodyLull,
            ProtocolBodyRefusal::NotPlaying,
        );
        assert_eq!(lull.stage, CancellationStage::BodyLull);
    }
}
