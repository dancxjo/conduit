//! Explicit policy admission of current Host offer detail for planning use.

use super::{PatchbayHtmlServer, ServerError};
use conduit_body::{
    ClaimUseClass, HostOfferProjection, OfferDisclosureStage, RemoteClaimPolicy,
    RemoteClaimProvenance, RemoteProofClass, MAX_DISCLOSED_CAPABILITIES, MAX_DISCLOSED_RESOURCES,
};
use conduit_core::{BaseImplementationId, HostAdvertisement, SignId, PROTOCOL_VERSION};
use std::net::TcpStream;

const MAX_BODY_HOST_PLANNING_OFFER_BYTES: usize = 32 * 1024;

impl PatchbayHtmlServer {
    pub(super) fn deliver_body_planning_requirements(
        &mut self,
        stream: &mut TcpStream,
    ) -> Result<(), ServerError> {
        let workset = &self
            .body_workload
            .as_ref()
            .ok_or_else(|| ServerError::Interaction("Body workload session is absent".into()))?
            .evidence()
            .body
            .workset;
        let requirements =
            patchbay_model::body_planning_requirements(workset, &self.body_planning_forms)
                .map_err(|error| {
                    ServerError::Interaction(format!("Body planning requirements: {error:?}"))
                })?;
        let body = serde_json::to_vec(&requirements)
            .map_err(|error| ServerError::Interaction(error.to_string()))?;
        super::write_response(stream, "200 OK", "application/json; charset=utf-8", &body)
    }

    pub(super) fn deliver_body_host_planning_offer(
        &mut self,
        stream: &mut TcpStream,
        bytes: &[u8],
    ) -> Result<(), ServerError> {
        let body = match self.apply_body_host_planning_offer(bytes) {
            Ok(body) => body,
            Err(ServerError::InvalidRequest) => {
                return super::write_response(
                    stream,
                    "400 Bad Request",
                    "text/plain; charset=utf-8",
                    b"invalid Body Host planning offer",
                );
            }
            Err(ServerError::Interaction(detail)) => {
                return super::write_response(
                    stream,
                    "409 Conflict",
                    "text/plain; charset=utf-8",
                    detail.as_bytes(),
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
        let advertisement = advertisement(&evidence)?;
        let workset = &biography.body.workset;
        let forms = patchbay_model::plan_body_workset_on_host(
            workset,
            &self.body_planning_forms,
            &advertisement,
            &[BaseImplementationId::from("conduit.base/local@1")],
        )
        .map_err(|error| ServerError::Interaction(format!("Body planning: {error:?}")))?;
        let sequence = self.snapshot.interaction.revision.saturating_add(1);
        let sign = |stage: &str| {
            SignId::from(format!(
                "patchbay-html/body-plan/{}/{sequence}/{stage}",
                evidence.host_id.as_str()
            ))
        };
        let next_planning = if let Some(current) = &self.body_planning {
            let mut next = current.clone();
            next.replan(
                forms,
                patchbay_model::BodyPlanningTransition {
                    unsatisfied_sign_id: Some(sign("prior-unsatisfied")),
                    plan_ready_sign_id: sign("replacement-ready"),
                    play_sequence: sequence,
                    play_started_sign_id: sign("replacement-playing"),
                },
            )
            .map_err(|error| ServerError::Interaction(format!("Body replan: {error:?}")))?;
            next
        } else {
            patchbay_model::BodyPlanningSession::start(
                &biography.body,
                sequence,
                sign("wake"),
                forms,
                sign("initial-ready"),
                sequence,
                sign("initial-playing"),
            )
            .map_err(|error| ServerError::Interaction(format!("Body initial plan: {error:?}")))?
        };
        self.snapshot.body_host_planning_offer = Some(evidence);
        self.snapshot.body_planning = Some(next_planning.snapshot());
        self.body_planning = Some(next_planning);
        self.snapshot.interaction.revision = self.snapshot.interaction.revision.saturating_add(1);
        self.snapshot.interaction.last_request_id =
            Some("body-host-offer/admit-for-planning".into());
        self.snapshot.interaction.last_disposition =
            Some("Succeeded(PlanningInputAdmitted;BodyReplanned)".into());
        self.encoded_snapshot = self.snapshot.encode()?;
        Ok(self.encoded_snapshot.clone())
    }
}

fn advertisement(evidence: &HostOfferProjection) -> Result<HostAdvertisement, ServerError> {
    Ok(HostAdvertisement {
        protocol_version: evidence.protocol_version,
        host_id: evidence.host_id.clone(),
        boot_id: evidence.boot_id.clone(),
        offer_generation: evidence.offer_generation,
        profile: evidence
            .profile
            .clone()
            .ok_or_else(|| ServerError::Interaction("planning offer profile is absent".into()))?,
        resources: evidence.resources.clone(),
        planner_capabilities: Vec::new(),
        capabilities: evidence.capabilities.clone(),
    })
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
        || (evidence.capabilities.is_empty() && evidence.resources.is_empty())
        || evidence.capabilities.iter().any(|offer| {
            offer.capability_id.as_str().is_empty()
                || offer.implementation.implementation_id.as_str().is_empty()
        })
        || evidence
            .capabilities
            .windows(2)
            .any(|pair| pair[0].capability_id >= pair[1].capability_id)
        || evidence
            .resources
            .windows(2)
            .any(|pair| pair[0].pool_id >= pair[1].pool_id)
}

fn matches_admitted_summary(
    evidence: &HostOfferProjection,
    summary: Option<&HostOfferProjection>,
) -> bool {
    summary.is_some_and(|summary| {
        summary.host_id == evidence.host_id
            && summary.boot_id == evidence.boot_id
            && summary.offer_generation == evidence.offer_generation
            && (summary.capability_summary.len() == MAX_DISCLOSED_CAPABILITIES
                || evidence.capabilities.iter().all(|offer| {
                    summary.capability_summary.iter().any(|item| {
                        item.capability_id == offer.capability_id
                            && item.implementation_id == offer.implementation.implementation_id
                    })
                }))
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
        let requirements = patchbay_model::body_planning_requirements(
            &server
                .body_workload
                .as_ref()
                .unwrap()
                .evidence()
                .body
                .workset,
            &server.body_planning_forms,
        )
        .unwrap();
        let mut offers = requirements
            .kind_ids
            .iter()
            .map(|kind| {
                host.advertisement()
                    .capabilities
                    .iter()
                    .find(|offer| &offer.kind_id == kind)
                    .unwrap()
                    .clone()
            })
            .collect::<Vec<_>>();
        offers.sort_by(|left, right| left.capability_id.cmp(&right.capability_id));
        offers.dedup_by(|left, right| left.capability_id == right.capability_id);
        let required_classes = offers
            .iter()
            .flat_map(|offer| {
                offer
                    .resource_requirements
                    .iter()
                    .map(|item| &item.class_id)
            })
            .collect::<Vec<_>>();
        let mut resources = host
            .advertisement()
            .resources
            .iter()
            .filter(|offer| required_classes.contains(&&offer.class_id))
            .cloned()
            .collect::<Vec<_>>();
        resources.sort_by(|left, right| left.pool_id.cmp(&right.pool_id));
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
            capability_summary: offers
                .iter()
                .map(|offer| CapabilitySummary {
                    capability_id: offer.capability_id.clone(),
                    implementation_id: offer.implementation.implementation_id.clone(),
                })
                .collect(),
            capabilities: Vec::new(),
            resources: Vec::new(),
        };
        let planning = HostOfferProjection {
            stage: OfferDisclosureStage::Planning,
            observation_sign_id: SignId::from("sign/browser/planning-detail"),
            freshness_sequence: 2,
            capability_summary: Vec::new(),
            capabilities: offers,
            resources,
            ..summary.clone()
        };
        (summary, planning)
    }

    #[test]
    fn explicit_policy_admission_starts_an_ordinary_body_plan() {
        let snapshot = crate::body_workbench_fixture_snapshot(false).unwrap();
        let body_id = snapshot.body_workbench.as_ref().unwrap().body_id.clone();
        let forms = crate::body_workbench_fixture_forms().unwrap();
        let mut server = PatchbayHtmlServer::bind_ephemeral(&snapshot)
            .unwrap()
            .with_body_planning_forms(forms)
            .unwrap();
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
        assert!(admitted.body_planning.is_some());
        assert_eq!(
            admitted.body_planning.as_ref().unwrap().body_id.as_str(),
            body_id
        );
        assert_eq!(
            admitted.interaction.last_disposition.as_deref(),
            Some("Succeeded(PlanningInputAdmitted;BodyReplanned)")
        );
    }

    #[test]
    fn stale_boot_and_detail_absent_from_summary_refuse_atomically() {
        let snapshot = crate::body_workbench_fixture_snapshot(false).unwrap();
        let forms = crate::body_workbench_fixture_forms().unwrap();
        let mut server = PatchbayHtmlServer::bind_ephemeral(&snapshot)
            .unwrap()
            .with_body_planning_forms(forms)
            .unwrap();
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
