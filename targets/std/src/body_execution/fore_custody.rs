//! One stopped Fore delivery retains its already-admitted payload slot.
use crate::ExternalForeDelivery;
use serde::Serialize;
use sha2::{Digest, Sha256};

/// Last established stage, never an assertion about an unobserved external effect.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum BodyForeDeliveryStage {
    /// No adapter was available; no call was attempted.
    NotAttempted,
    /// The adapter returned an error. It may already have performed the effect.
    AdapterOutcomeUnknown,
    /// The adapter accepted; kernel acceptance did not complete.
    AdapterAccepted,
    /// Kernel acceptance completed; delivery acknowledgement did not complete.
    KernelAccepted,
}

/// Custody belongs to the containing report's exact Plan/Play/terminal Sign.
/// This is not a retry token or permission to replay the value in a new Play.
#[derive(Debug)]
pub struct BodyForePendingOutput {
    pub delivery: ExternalForeDelivery,
    pub stage: BodyForeDeliveryStage,
}

/// Compact post-execution receipt; payload custody remains in the run report.
#[derive(Debug, Serialize)]
pub struct BodyForePendingOutputMetadata<'a> {
    pub stage: BodyForeDeliveryStage,
    pub front_port_id: &'a conduit_core::PortId,
    pub track: &'a conduit_core::ConnectionTrack,
    pub value_kind: &'a conduit_core::KindId,
    pub sequence: u64,
    pub byte_length: usize,
    pub sha256: String,
}

impl BodyForePendingOutput {
    /// Build a receipt after execution. Hashing/formatting is not Play work or
    /// external acknowledgement, and never turns unknown delivery into success.
    pub fn metadata(&self) -> BodyForePendingOutputMetadata<'_> {
        BodyForePendingOutputMetadata {
            stage: self.stage,
            front_port_id: &self.delivery.front_port_id,
            track: &self.delivery.track,
            value_kind: &self.delivery.value_kind,
            sequence: self.delivery.sequence,
            byte_length: self.delivery.bytes.len(),
            sha256: format!("{:x}", Sha256::digest(self.delivery.bytes.as_slice())),
        }
    }
}
