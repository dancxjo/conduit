use super::*;

pub(super) fn validate(frame: &BrowserAdmissionEgress) -> Result<(), BrowserAdmissionFrameError> {
    let protocol = match frame {
        BrowserAdmissionEgress::Challenge { protocol, .. }
        | BrowserAdmissionEgress::Admitted { protocol, .. }
        | BrowserAdmissionEgress::PresenceAccepted { protocol, .. }
        | BrowserAdmissionEgress::ReturnChallenge { protocol, .. }
        | BrowserAdmissionEgress::Refused { protocol, .. } => protocol,
        BrowserAdmissionEgress::BiographyEvidence { protocol, evidence } => {
            evidence
                .validate()
                .map_err(|_| BrowserAdmissionFrameError::InvalidBiographyEvidence)?;
            protocol
        }
        BrowserAdmissionEgress::OfferEvidence { protocol, evidence } => {
            offer_evidence::validate(evidence)?;
            protocol
        }
        BrowserAdmissionEgress::MediaUsePlan {
            protocol,
            plan_id,
            resource_handle,
            output_port,
        } => {
            if plan_id.as_str().is_empty()
                || resource_handle.as_str().is_empty()
                || output_port.as_str().is_empty()
            {
                return Err(BrowserAdmissionFrameError::InvalidMediaResource);
            }
            protocol
        }
        BrowserAdmissionEgress::WebRtcPlanReady {
            protocol,
            generation,
            plan_id,
        } => {
            if *generation == 0
                || *generation >= MAX_WEBRTC_GRANT_GENERATIONS
                || plan_id.as_str().is_empty()
            {
                return Err(BrowserAdmissionFrameError::InvalidGrant);
            }
            protocol
        }
        BrowserAdmissionEgress::WebRtcSignal {
            protocol, signal, ..
        } => {
            signal.validate()?;
            protocol
        }
        BrowserAdmissionEgress::WebRtcGrant {
            protocol,
            generation,
            index,
            total,
            grant,
        } => {
            if *generation >= MAX_WEBRTC_GRANT_GENERATIONS
                || usize::from(*index) >= MAX_WEBRTC_NEGOTIATIONS
                || usize::from(*total) > MAX_WEBRTC_NEGOTIATIONS
                || grant.is_some() != (*index < *total)
            {
                return Err(BrowserAdmissionFrameError::InvalidGrant);
            }
            if let Some(grant) = grant {
                grant.validate()?;
            }
            protocol
        }
    };
    (*protocol == BROWSER_ADMISSION_PROTOCOL)
        .then_some(())
        .ok_or(BrowserAdmissionFrameError::WrongProtocol)
}
