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
        BrowserAdmissionEgress::FaceSnapshotResponse { protocol, response } => {
            if serde_json::to_vec(response)
                .map_err(|_| BrowserAdmissionFrameError::InvalidFaceSnapshot)?
                .len()
                > MAX_OWNER_FACE_RESPONSE_BYTES
            {
                return Err(BrowserAdmissionFrameError::InvalidFaceSnapshot);
            }
            match response {
                OwnerFaceSnapshotResponse::Snapshot {
                    schema,
                    presentation,
                    interactions_admitted,
                    route,
                    ..
                } if schema == OWNER_FACE_RESPONSE_SCHEMA => {
                    presentation
                        .validate()
                        .map_err(|_| BrowserAdmissionFrameError::InvalidFaceSnapshot)?;
                    if let Some(route) = route {
                        route
                            .verify_seal()
                            .map_err(|_| BrowserAdmissionFrameError::InvalidFaceSnapshot)?;
                        if route.face_id != presentation.identity
                            || route.face_revision != presentation.revision
                        {
                            return Err(BrowserAdmissionFrameError::InvalidFaceSnapshot);
                        }
                    } else if *interactions_admitted {
                        return Err(BrowserAdmissionFrameError::InvalidFaceSnapshot);
                    }
                }
                OwnerFaceSnapshotResponse::Unchanged {
                    schema, identity, ..
                } if schema == OWNER_FACE_RESPONSE_SCHEMA && !identity.as_str().is_empty() => {}
                OwnerFaceSnapshotResponse::Refused { schema, code }
                    if schema == OWNER_FACE_RESPONSE_SCHEMA
                        && !code.is_empty()
                        && code.len() <= 128 => {}
                _ => return Err(BrowserAdmissionFrameError::InvalidFaceSnapshot),
            }
            protocol
        }
        BrowserAdmissionEgress::FaceInteractionResponse {
            protocol,
            accepted,
            code,
        }
        | BrowserAdmissionEgress::FaceShowResponse {
            protocol,
            accepted,
            code,
        } => {
            if code.len() > 128
                || (*accepted && !code.is_empty())
                || (!*accepted && code.is_empty())
            {
                return Err(BrowserAdmissionFrameError::InvalidFaceSnapshot);
            }
            protocol
        }
        BrowserAdmissionEgress::FaceWardrobeResponse {
            protocol,
            request_id,
            accepted,
            code,
            report,
        } => {
            if !valid_wardrobe_request_id(request_id)
                || code.len() > 128
                || (*accepted && (!code.is_empty() || report.is_none()))
                || (!*accepted && (code.is_empty() || report.is_some()))
                || serde_json::to_vec(report)
                    .map_err(|_| BrowserAdmissionFrameError::InvalidFaceSnapshot)?
                    .len()
                    > 64 * 1024
            {
                return Err(BrowserAdmissionFrameError::InvalidFaceSnapshot);
            }
            protocol
        }
        BrowserAdmissionEgress::SelectedSpeechResponse {
            protocol,
            request_id,
            outcome,
            operation_id,
            status,
            code,
        } => {
            let shape = match outcome.as_str() {
                "started" | "stop-requested" => {
                    operation_id.is_some() && status.is_none() && code.is_none()
                }
                "status" => operation_id.is_some() && status.is_some() && code.is_none(),
                "refused" => status.is_none() && code.is_some(),
                _ => false,
            };
            if !shape
                || !valid_speech_id(request_id)
                || operation_id.as_ref().is_some_and(|id| !valid_speech_id(id))
                || code
                    .as_ref()
                    .is_some_and(|value| value.is_empty() || value.len() > 128)
                || serde_json::to_vec(status)
                    .map_err(|_| BrowserAdmissionFrameError::InvalidFaceSnapshot)?
                    .len()
                    > 64 * 1024
            {
                return Err(BrowserAdmissionFrameError::InvalidFaceSnapshot);
            }
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
            if *generation == 0 || plan_id.as_str().is_empty() {
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
            index,
            total,
            grant,
            ..
        } => {
            if usize::from(*index) >= MAX_WEBRTC_NEGOTIATIONS
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
