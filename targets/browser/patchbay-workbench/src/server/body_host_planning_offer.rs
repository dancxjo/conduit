//! Explicit policy admission of current host offer detail for planning use.

use super::{PatchbayHtmlServer, ServerError};
use conduit_body::{
    ClaimUseClass, HostOfferProjection, OfferDisclosureStage, RemoteClaimPolicy,
    RemoteClaimProvenance, RemoteProofClass, MAX_DISCLOSED_CAPABILITIES, MAX_DISCLOSED_RESOURCES,
};
use conduit_core::{BaseImplementationId, HostAdvertisement, SignId, PROTOCOL_VERSION};
use std::net::TcpStream;

// The complete ordinary keyboard planning disclosure measures 427,250 bytes.
pub(super) const MAX_BODY_HOST_PLANNING_OFFER_BYTES: usize = 1024 * 1024;

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
        let plots = planning_plots(workset, &self.body_planning_plots)?;
        let requirements =
            conduit_body_make::body_planning_requirements(workset, &plots).map_err(|error| {
                ServerError::Interaction(format!("Body planning requirements: {error:?}"))
            })?;
        let mut requirements = serde_json::to_value(requirements)
            .map_err(|error| ServerError::Interaction(error.to_string()))?;
        requirements["mask_kind_ids"] = serde_json::json!(if self.selected_browser_mask {
            vec!["face/browser-dom-mask"]
        } else {
            Vec::<&str>::new()
        });
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
                "planning offer is not current admitted host evidence".into(),
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
        let candidates = planning_plots(workset, &self.body_planning_plots)?;
        let plots = conduit_body_make::plan_body_workset_on_host(
            workset,
            &candidates,
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
        let mut next_planning = if let Some(current) = &self.body_planning {
            let mut next = current.clone();
            if current.wake().lifecycle == conduit_body::WakeLifecycle::Lulled {
                next.prepare_next_wake(&biography.body, sequence, sign("wake"), plots)
                    .map_err(|error| {
                        ServerError::Interaction(format!("Body next Wake: {error:?}"))
                    })?;
            } else {
                next.replace_proposal(plots)
                    .map_err(|error| ServerError::Interaction(format!("Body replan: {error:?}")))?;
            }
            next
        } else {
            conduit_body::BodyPlanningSession::prepare(
                &biography.body,
                sequence,
                sign("wake"),
                plots,
            )
            .map_err(|error| ServerError::Interaction(format!("Body initial plan: {error:?}")))?
        };
        let mut next_mask_control = self.mask_control.clone();
        if next_planning.current_plan().mask_topologies.is_empty() {
            if let Some(control) = next_mask_control.as_mut() {
                if self.selected_browser_mask {
                    let planned = conduit_browser_mask_offer::planned_mask(
                        &advertisement,
                        conduit_browser_mask_offer::MASK_SOURCE,
                        "browser-graphical",
                    )
                    .map_err(|error| {
                        ServerError::Interaction(format!("selected browser Mask: {error}"))
                    })?;
                    control.install_initial_selected_graphical(planned, &mut next_planning)
                } else {
                    control.install_initial_graphical(&mut next_planning)
                }
                .map_err(|error| {
                    ServerError::Interaction(format!("Mask initial plan: {error:?}"))
                })?;
            }
        }
        let session = self
            .body_workload
            .as_ref()
            .ok_or_else(|| ServerError::Interaction("Body workload session is absent".into()))?;
        let (session, mut snapshot) =
            super::body_execution::history::retain(&self.snapshot, session, &next_planning)?;
        snapshot.body_host_planning_offer = Some(evidence);
        snapshot.body_planning = Some(next_planning.snapshot());
        snapshot.interaction.revision = snapshot
            .interaction
            .revision
            .checked_add(1)
            .ok_or_else(|| ServerError::Interaction("Body planning revision exhausted".into()))?;
        snapshot.interaction.last_request_id = Some("body-host-offer/admit-for-planning".into());
        snapshot.interaction.last_disposition =
            Some("Succeeded(PlanningInputAdmitted;BodyReplanned)".into());
        let encoded = snapshot.encode()?;
        let navigation = super::navigation_state(&snapshot)?;
        self.body_workload = Some(session);
        self.body_planning = Some(next_planning);
        self.mask_control = next_mask_control;
        self.body_face_show = None;
        self.snapshot = snapshot;
        self.navigation = navigation;
        self.encoded_snapshot = encoded;
        Ok(self.encoded_snapshot.clone())
    }
}

pub(super) fn planning_plots(
    workset: &conduit_body::BodyWorkset,
    candidates: &[conduit_patchbay_workbench::PlotCandidate],
) -> Result<Vec<conduit_body_make::BodyPlanningPlot>, ServerError> {
    workset
        .plots()
        .iter()
        .map(|resident| {
            candidates
                .iter()
                .find(|candidate| {
                    candidate.source_document_id == resident.source_document_id
                        && candidate.checked_plot_id == resident.checked_plot_id
                })
                .ok_or_else(|| ServerError::Interaction("Body planning plots: MissingPlot".into()))?
                .body_planning_plot()
                .map_err(|error| {
                    ServerError::Interaction(format!("Body planning plots: {error:?}"))
                })
        })
        .collect::<Result<Vec<_>, _>>()
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
        bases: vec![],
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
mod tests;
