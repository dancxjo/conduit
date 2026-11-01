//! Commit-last correspondence for original retired scan evidence. Storage must
//! itself have ordinary independent admission; this grants no erase or renewal.
use super::*;
use crate::recovery_evidence as wire;

#[must_use = "retain the actual committed evidence through Root retirement"]
pub struct CommittedScanEvidence {
    evidence: RetiredScanEvidence,
    receipt: wire::Committed,
    evidence_seen: bool,
}
impl CommittedScanEvidence {
    pub fn evidence(&self) -> &RetiredScanEvidence {
        &self.evidence
    }
    pub fn receipt(&self) -> &wire::Committed {
        &self.receipt
    }
    pub fn evidence_seen(&self) -> bool {
        self.evidence_seen
    }
}
impl RetiredScanEvidence {
    /// The opaque result is created only after exact encoding, commit-last
    /// programming and readback succeed for this original revoked possession.
    /// Lost readback retains the original value; retry deduplicates exact bytes.
    #[allow(clippy::result_large_err)]
    pub fn commit<S: wire::Storage>(
        self,
        active: &ActivePlayIdentity,
        evidence_seen: bool,
        signs: &conduit_kernel::HostedSignLog,
        journal: &mut wire::Journal<S>,
        work: &mut [u8; wire::MAXIMUM_BYTES],
    ) -> Result<CommittedScanEvidence, (Self, wire::Refusal)> {
        let mut append = || {
            let inspection = self.inspection();
            let scope = &inspection.scope;
            if inspection.lifecycle != CapabilityLifecycle::Revoked
                || inspection.in_flight_operations != 0
                || !active.is_bound_to(
                    &scope.plan_id,
                    &scope.host_id,
                    &scope.boot_id,
                    active.play_sequence,
                )
                || active.active_play_id != scope.active_play_id
                || scope.operation_contract_id.as_str() != NETWORK_SCAN_HOST_CALL
                || scope.authority_contract_id.as_str() != NETWORK_SCAN_AUTHORITY
                || scope.subject_kind.as_str() != NETWORK_SCAN_SUBJECT
                || scope.base_provider_generation != self.request().generation
            {
                return Err(wire::Refusal::Scope);
            }
            wire::encode_scan(
                wire::Scope {
                    host: scope.host_id.as_str(),
                    boot: scope.boot_id.as_str(),
                    plan: scope.plan_id.as_str(),
                    active_play: scope.active_play_id.as_str(),
                    provider_generation: scope.base_provider_generation,
                    play_sequence: active.play_sequence,
                    configuration_revision: 0,
                },
                signs.events(),
                signs,
                wire::ScanRecord {
                    request: self.request(),
                    evidence_seen,
                    output: self.completed_output().unwrap_or(&[]),
                },
                work,
            )?;
            journal.append(work)
        };
        match append() {
            Ok(receipt) => Ok(CommittedScanEvidence {
                evidence: self,
                receipt,
                evidence_seen,
            }),
            Err(error) => Err((self, error)),
        }
    }
}
