//! Move the actual Birth admission owner; a biography is not issuer custody.
use conduit_body::{AdmissionManager, BodyBiographyEvidence, BodyId};
use conduit_core::{BootId, HostId};

/// Instance-local custody, deliberately neither Clone nor serde data.
pub(crate) struct WorkspaceIssuerCustody {
    body: BodyId,
    host: HostId,
    boot: BootId,
}

impl WorkspaceIssuerCustody {
    pub(crate) fn matches(
        &self,
        body: &BodyBiographyEvidence,
        host: &HostId,
        boot: &BootId,
    ) -> bool {
        self.matches_identity(&body.body_id, host, boot)
    }

    pub(crate) fn matches_identity(&self, body: &BodyId, host: &HostId, boot: &BootId) -> bool {
        &self.body == body && &self.host == host && &self.boot == boot
    }
}

/// Called only after the receiving Root has prepared its complete response.
/// Failure retains the source owner. Success moves its manager exactly once.
pub(crate) fn handoff_workspace(
    expected: &BodyBiographyEvidence,
) -> Result<(AdmissionManager, Option<WorkspaceIssuerCustody>), String> {
    let current = super::super::session::take_for_workspace(expected)?;
    let issuer = if current.birth_issuer_custody {
        current
            .receipt
            .host_id
            .zip(current.receipt.boot_id)
            .map(|(host, boot)| WorkspaceIssuerCustody {
                body: current.biography.body_id.clone(),
                host: host.into(),
                boot: boot.into(),
            })
    } else {
        None
    };
    Ok((current.admission, issuer))
}
