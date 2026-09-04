//! Explicit policy admission of current Host offer detail for planning use.

use super::{PatchbayHtmlServer, ServerError};
use conduit_body::{
    ClaimUseClass, HostOfferProjection, OfferDisclosureStage, RemoteClaimPolicy,
    RemoteClaimProvenance, RemoteProofClass, MAX_DISCLOSED_CAPABILITIES, MAX_DISCLOSED_RESOURCES,
};
use conduit_core::PROTOCOL_VERSION;
use std::net::TcpStream;

const MAX_BODY_HOST_PLANNING_OFFER_BYTES: usize = 32 * 1024;

impl PatchbayHtmlServer {
    pub(super) fn deliver_body_host_planning_offer(
        &mut self,
        stream: &mut TcpStream,
        bytes: &[u8],
    ) -> Result<(), ServerError> {
        let body = match self.apply_body_host_planning_offer(bytes) {
            Ok(body) => body,
            Err(ServerError::InvalidRequest | ServerError::Interaction(_)) => {
                return super::write_response(
                    stream,
                    "400 Bad Request",
                    "text/plain; charset=utf-8",
                    b"invalid Body Host planning offer",
                );
            }
            Err(error) => return Err(error),
        };
        super::write_response(stream, "200 OK", "application/json; charset=utf-8", &body)
    }

    pub(super) fn apply_body_host_planning_offer(
        &mut self,
        bytes: &[u8],
    ) -> Result<Vec<u8>, ServerError> {
        if bytes.is_empty() || bytes.len() > MAX_BODY_HOST_PLANNING_OFFER_BYTES {
            return Err(ServerError::InvalidRequest);
        }
        let evidence: HostOfferProjection =
            serde_json::from_slice(bytes).map_err(|_| ServerError::InvalidRequest)?;
        let biography = self
            .body_workload
            .as_ref()
            .ok_or_else(|| ServerError::Interaction("Body workload session is absent".into()))?
            .evidence();
        let membership = &biography.membership;
        if invalid_projection(&evidence)
            || !super::body_host_offer_evidence::is_current(&evidence, biography)
            || !matches_admitted_summary(&evidence, self.snapshot.body_host_offer_evidence.as_ref())
        {
            return Err(ServerError::Interaction(
                "planning offer is not current admitted Host evidence".into(),
            ));
        }
        let policy = RemoteClaimPolicy {
            accepted_sources: vec![evidence.host_id.clone()],
            accepted_proof_classes: vec![RemoteProofClass::SelfReported],
            require_current_member: true,
            minimum_independent_sources: 1,
            use_class: ClaimUseClass::Planning,
        };
        for provenance in provenance(&evidence) {
            policy
                .admits(&provenance, Some(membership), ClaimUseClass::Planning)
                .map_err(|error| ServerError::Interaction(format!("planning policy: {error:?}")))?;
        }
        self.snapshot.body_host_planning_offer = Some(evidence);
        self.snapshot.interaction.revision = self.snapshot.interaction.revision.saturating_add(1);
        self.snapshot.interaction.last_request_id =
            Some("body-host-offer/admit-for-planning".into());
        self.snapshot.interaction.last_disposition =
            Some("Succeeded(PlanningInputAdmitted;ReplanNotRequested)".into());
        self.encoded_snapshot = self.snapshot.encode()?;
        Ok(self.encoded_snapshot.clone())
    }
}

pub(crate) fn invalid_projection(evidence: &HostOfferProjection) -> bool {
    evidence.stage != OfferDisclosureStage::Planning
        || evidence.protocol_version != PROTOCOL_VERSION
        || evidence.host_id.as_str().is_empty()
        || evidence.boot_id.as_str().is_empty()
        || evidence.observation_sign_id.as_str().is_empty()
        || evidence.freshness_sequence == 0
        || evidence.proof_class != RemoteProofClass::SelfReported
        || evidence.profile.is_none()
        || !evidence.capability_summary.is_empty()
        || evidence.capabilities.len() > MAX_DISCLOSED_CAPABILITIES
        || evidence.resources.len() > MAX_DISCLOSED_RESOURCES
        || !evidence.resources.is_empty()
        || (evidence.capabilities.is_empty() && evidence.resources.is_empty())
        || evidence.capabilities.iter().any(|offer| {
            offer.capability_id.as_str().is_empty()
                || offer.implementation.implementation_id.as_str().is_empty()
        })
        || evidence
            .capabilities
            .windows(2)
            .any(|pair| pair[0].capability_id >= pair[1].capability_id)
}

fn matches_admitted_summary(
    evidence: &HostOfferProjection,
    summary: Option<&HostOfferProjection>,
) -> bool {
    summary.is_some_and(|summary| {
        summary.host_id == evidence.host_id
            && summary.boot_id == evidence.boot_id
            && summary.offer_generation == evidence.offer_generation
            && evidence.capabilities.iter().all(|offer| {
                summary.capability_summary.iter().any(|item| {
                    item.capability_id == offer.capability_id
                        && item.implementation_id == offer.implementation.implementation_id
                })
            })
    })
}

fn provenance(evidence: &HostOfferProjection) -> Vec<RemoteClaimProvenance> {
    let base = |capability_id, implementation_id, resource_pool_id| RemoteClaimProvenance {
        asserting_host_id: evidence.host_id.clone(),
        asserting_boot_id: evidence.boot_id.clone(),
        offer_generation: evidence.offer_generation,
        capability_id,
        implementation_id,
        base_id: None,
        resource_pool_id,
        plan_id: None,
        active_play_id: None,
        sign_id: evidence.observation_sign_id.clone(),
        freshness_sequence: evidence.freshness_sequence,
        proof_class: evidence.proof_class,
    };
    evidence
        .capabilities
        .iter()
        .map(|offer| {
            base(
                Some(offer.capability_id.clone()),
                Some(offer.implementation.implementation_id.clone()),
                None,
            )
        })
        .chain(
            evidence
                .resources
                .iter()
                .map(|offer| base(None, None, Some(offer.pool_id.clone()))),
        )
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use conduit_body::CapabilitySummary;
    use conduit_core::{CapabilityId, HostProfileId, ImplementationId, SignId};

    fn projections(server: &PatchbayHtmlServer) -> (HostOfferProjection, HostOfferProjection) {
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
        let host = conduit_std_host::StdHost::new();
        let offer = host
            .advertisement()
            .capabilities
            .iter()
            .find(|offer| offer.kind_id.as_str() == "text/upper")
            .unwrap()
            .clone();
        let summary = HostOfferProjection {
            stage: OfferDisclosureStage::AdmittedMembership,
            protocol_version: PROTOCOL_VERSION,
            host_id: current.host_id.clone(),
            boot_id: current.boot_id.clone(),
            offer_generation: current.offer_generation,
            observation_sign_id: SignId::from("sign/browser/offer-summary"),
            freshness_sequence: 1,
            proof_class: RemoteProofClass::SelfReported,
            profile: Some(HostProfileId::from("browser-standard")),
            capability_summary: vec![CapabilitySummary {
                capability_id: offer.capability_id.clone(),
                implementation_id: offer.implementation.implementation_id.clone(),
            }],
            capabilities: Vec::new(),
            resources: Vec::new(),
        };
        let planning = HostOfferProjection {
            stage: OfferDisclosureStage::Planning,
            observation_sign_id: SignId::from("sign/browser/planning-detail"),
            freshness_sequence: 2,
            capability_summary: Vec::new(),
            capabilities: vec![offer],
            ..summary.clone()
        };
        (summary, planning)
    }

    #[test]
    fn explicit_policy_admits_only_current_detail_without_inventing_a_plan() {
        let snapshot = crate::body_workbench_fixture_snapshot(false).unwrap();
        let body_id = snapshot.body_workbench.as_ref().unwrap().body_id.clone();
        let mut server = PatchbayHtmlServer::bind_ephemeral(&snapshot).unwrap();
        let (summary, planning) = projections(&server);
        server
            .apply_body_host_offer_evidence(&serde_json::to_vec(&summary).unwrap())
            .unwrap();

        let admitted: crate::RendererSnapshot = serde_json::from_slice(
            &server
                .apply_body_host_planning_offer(&serde_json::to_vec(&planning).unwrap())
                .unwrap(),
        )
        .unwrap();
        assert_eq!(admitted.body_host_planning_offer.as_ref(), Some(&planning));
        assert_eq!(admitted.body_workbench.as_ref().unwrap().body_id, body_id);
        assert!(admitted.presentation.basis.plan_id.is_none());
        assert_eq!(
            admitted.interaction.last_disposition.as_deref(),
            Some("Succeeded(PlanningInputAdmitted;ReplanNotRequested)")
        );
    }

    #[test]
    fn stale_boot_and_detail_absent_from_summary_refuse_atomically() {
        let snapshot = crate::body_workbench_fixture_snapshot(false).unwrap();
        let mut server = PatchbayHtmlServer::bind_ephemeral(&snapshot).unwrap();
        let (summary, mut planning) = projections(&server);
        server
            .apply_body_host_offer_evidence(&serde_json::to_vec(&summary).unwrap())
            .unwrap();
        let prior = server.encoded_snapshot.clone();

        planning.boot_id = conduit_core::BootId::from("stale/browser-boot");
        assert!(server
            .apply_body_host_planning_offer(&serde_json::to_vec(&planning).unwrap())
            .is_err());
        assert_eq!(server.encoded_snapshot, prior);

        let (_, mut planning) = projections(&server);
        planning.capabilities[0].capability_id = CapabilityId::from("unadvertised-capability");
        planning.capabilities[0].implementation.implementation_id =
            ImplementationId::from("unadvertised-implementation");
        assert!(server
            .apply_body_host_planning_offer(&serde_json::to_vec(&planning).unwrap())
            .is_err());
        assert_eq!(server.encoded_snapshot, prior);
    }
}
