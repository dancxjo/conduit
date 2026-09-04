//! Bounded adoption of current, display-only Host offer evidence.

use super::{PatchbayHtmlServer, ServerError};
use conduit_body::{
    BodyBiographyEvidence, HostOfferProjection, MembershipState, OfferDisclosureStage,
    RemoteProofClass, MAX_DISCLOSED_CAPABILITIES,
};
use conduit_core::PROTOCOL_VERSION;
use std::net::TcpStream;

const MAX_BODY_HOST_OFFER_EVIDENCE_BYTES: usize = 32 * 1024;

impl PatchbayHtmlServer {
    pub(super) fn deliver_body_host_offer_evidence(
        &mut self,
        stream: &mut TcpStream,
        bytes: &[u8],
    ) -> Result<(), ServerError> {
        let body = match self.apply_body_host_offer_evidence(bytes) {
            Ok(body) => body,
            Err(ServerError::InvalidRequest | ServerError::Interaction(_)) => {
                return super::write_response(
                    stream,
                    "400 Bad Request",
                    "text/plain; charset=utf-8",
                    b"invalid Body Host offer evidence",
                );
            }
            Err(error) => return Err(error),
        };
        super::write_response(stream, "200 OK", "application/json; charset=utf-8", &body)
    }

    pub(super) fn apply_body_host_offer_evidence(
        &mut self,
        bytes: &[u8],
    ) -> Result<Vec<u8>, ServerError> {
        if bytes.is_empty() || bytes.len() > MAX_BODY_HOST_OFFER_EVIDENCE_BYTES {
            return Err(ServerError::InvalidRequest);
        }
        let evidence: HostOfferProjection =
            serde_json::from_slice(bytes).map_err(|_| ServerError::InvalidRequest)?;
        let biography = self
            .body_workload
            .as_ref()
            .ok_or_else(|| ServerError::Interaction("Body workload session is absent".into()))?
            .evidence();
        if invalid_projection(&evidence) || !is_current(&evidence, biography) {
            return Err(ServerError::Interaction(
                "Host offer evidence is not current admitted membership".into(),
            ));
        }
        if self
            .snapshot
            .body_host_offer_evidence
            .as_ref()
            .is_some_and(|prior| {
                prior.host_id == evidence.host_id
                    && prior.boot_id == evidence.boot_id
                    && prior.offer_generation == evidence.offer_generation
                    && prior.freshness_sequence >= evidence.freshness_sequence
            })
        {
            return Err(ServerError::Interaction(
                "Host offer evidence does not advance freshness".into(),
            ));
        }
        self.snapshot.body_host_offer_evidence = Some(evidence);
        self.snapshot.interaction.revision = self.snapshot.interaction.revision.saturating_add(1);
        self.snapshot.interaction.last_request_id = Some("body-host-offer/adopt".into());
        self.snapshot.interaction.last_disposition = Some("Succeeded(DisplayOrDiagnostic)".into());
        self.encoded_snapshot = self.snapshot.encode()?;
        Ok(self.encoded_snapshot.clone())
    }
}

pub(crate) fn invalid_projection(evidence: &HostOfferProjection) -> bool {
    evidence.stage != OfferDisclosureStage::AdmittedMembership
        || evidence.protocol_version != PROTOCOL_VERSION
        || evidence.host_id.as_str().is_empty()
        || evidence.boot_id.as_str().is_empty()
        || evidence.observation_sign_id.as_str().is_empty()
        || evidence.freshness_sequence == 0
        || evidence.proof_class != RemoteProofClass::SelfReported
        || evidence.profile.is_none()
        || evidence.capability_summary.len() > MAX_DISCLOSED_CAPABILITIES
        || !evidence.capabilities.is_empty()
        || !evidence.resources.is_empty()
        || evidence.capability_summary.iter().any(|summary| {
            summary.capability_id.as_str().is_empty()
                || summary.implementation_id.as_str().is_empty()
        })
        || evidence
            .capability_summary
            .windows(2)
            .any(|pair| pair[0].capability_id >= pair[1].capability_id)
}

pub(crate) fn is_current(
    evidence: &HostOfferProjection,
    biography: &BodyBiographyEvidence,
) -> bool {
    biography.membership.parts.iter().any(|part| {
        part.state == MembershipState::Admitted
            && part.current.as_ref().is_some_and(|current| {
                current.host_id == evidence.host_id
                    && current.boot_id == evidence.boot_id
                    && current.offer_generation == evidence.offer_generation
            })
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use conduit_core::{HostProfileId, SignId};

    fn current_projection(server: &PatchbayHtmlServer) -> HostOfferProjection {
        let current = server
            .body_workload
            .as_ref()
            .unwrap()
            .evidence()
            .membership
            .parts
            .iter()
            .find_map(|part| part.current.as_ref())
            .unwrap();
        HostOfferProjection {
            stage: OfferDisclosureStage::AdmittedMembership,
            protocol_version: PROTOCOL_VERSION,
            host_id: current.host_id.clone(),
            boot_id: current.boot_id.clone(),
            offer_generation: current.offer_generation,
            observation_sign_id: SignId::from("sign/browser/admitted-offer"),
            freshness_sequence: 1,
            proof_class: RemoteProofClass::SelfReported,
            profile: Some(HostProfileId::from("browser-standard")),
            capability_summary: vec![],
            capabilities: vec![],
            resources: vec![],
        }
    }

    #[test]
    fn current_self_report_is_display_only_and_replay_refuses_atomically() {
        let snapshot = crate::body_workbench_fixture_snapshot(false).unwrap();
        let mut server = PatchbayHtmlServer::bind_ephemeral(&snapshot).unwrap();
        let evidence = current_projection(&server);
        let encoded = serde_json::to_vec(&evidence).unwrap();

        let adopted: crate::RendererSnapshot =
            serde_json::from_slice(&server.apply_body_host_offer_evidence(&encoded).unwrap())
                .unwrap();
        assert_eq!(adopted.body_host_offer_evidence.as_ref(), Some(&evidence));
        assert_eq!(
            adopted.interaction.last_disposition.as_deref(),
            Some("Succeeded(DisplayOrDiagnostic)")
        );
        let prior = server.encoded_snapshot.clone();
        assert!(server.apply_body_host_offer_evidence(&encoded).is_err());
        assert_eq!(server.encoded_snapshot, prior);
    }

    #[test]
    fn wrong_boot_and_non_self_report_refuse_without_adoption() {
        let snapshot = crate::body_workbench_fixture_snapshot(false).unwrap();
        let mut server = PatchbayHtmlServer::bind_ephemeral(&snapshot).unwrap();
        let mut evidence = current_projection(&server);
        evidence.boot_id = conduit_core::BootId::from("wrong-boot");
        assert!(server
            .apply_body_host_offer_evidence(&serde_json::to_vec(&evidence).unwrap())
            .is_err());
        evidence = current_projection(&server);
        evidence.proof_class = RemoteProofClass::PlatformObserved;
        assert!(server
            .apply_body_host_offer_evidence(&serde_json::to_vec(&evidence).unwrap())
            .is_err());
        assert!(server.snapshot.body_host_offer_evidence.is_none());
    }
}
