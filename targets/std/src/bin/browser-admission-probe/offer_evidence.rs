use conduit_body::{
    disclose_host_offer, BodyBiographyEvidence, CandidateId, CandidateInventory,
    CandidateObservation, OfferDisclosureRequest, OfferDisclosureStage, RemoteProofClass,
};
use conduit_std_host::browser_admission::{
    BrowserAdmissionEgress, BrowserAdmissionSocket, BROWSER_ADMISSION_PROTOCOL,
};

pub(super) fn send_admitted_offer_evidence(
    socket: &mut BrowserAdmissionSocket,
    observation: &CandidateObservation,
) -> Result<(), String> {
    let evidence = disclose_host_offer(
        observation,
        RemoteProofClass::SelfReported,
        &OfferDisclosureRequest {
            stage: OfferDisclosureStage::AdmittedMembership,
            capability_ids: vec![],
            resource_pool_ids: vec![],
        },
    )
    .map_err(|error| format!("disclose admitted browser offer: {error:?}"))?;
    socket
        .send(&BrowserAdmissionEgress::OfferEvidence {
            protocol: BROWSER_ADMISSION_PROTOCOL,
            evidence: Box::new(evidence),
        })
        .map_err(|error| format!("send admitted browser offer evidence: {error:?}"))
}

pub(super) fn observation_for_candidate(
    inventory: &CandidateInventory,
    candidate_id: &CandidateId,
) -> Result<CandidateObservation, String> {
    inventory
        .candidates
        .iter()
        .find(|candidate| &candidate.candidate_id == candidate_id)
        .map(|candidate| candidate.observation.clone())
        .ok_or_else(|| "observed browser candidate disappeared".into())
}

pub(super) fn send_admission_evidence(
    socket: &mut BrowserAdmissionSocket,
    biography: &BodyBiographyEvidence,
    observation: &CandidateObservation,
) -> Result<(), String> {
    socket
        .send(&BrowserAdmissionEgress::BiographyEvidence {
            protocol: BROWSER_ADMISSION_PROTOCOL,
            evidence: Box::new(biography.clone()),
        })
        .map_err(|error| format!("send biography evidence: {error:?}"))?;
    send_admitted_offer_evidence(socket, observation)
}
