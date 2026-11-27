//! Exact browser egress wire alternatives; membership and native responses stay distinct.
use super::*;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "kebab-case", deny_unknown_fields)]
pub enum BrowserAdmissionEgress {
    Challenge {
        protocol: u16,
        challenge: AdmissionChallenge,
    },
    Admitted {
        protocol: u16,
        credential: MembershipCredential,
    },
    BiographyEvidence {
        protocol: u16,
        evidence: Box<BodyBiographyEvidence>,
    },
    OfferEvidence {
        protocol: u16,
        evidence: Box<HostOfferProjection>,
    },
    PresenceAccepted {
        protocol: u16,
        sequence: u64,
        renew_after_millis: u64,
        expires_at_millis: u64,
    },
    FaceSnapshotResponse {
        protocol: u16,
        response: OwnerFaceSnapshotResponse,
    },
    FaceShowResponse {
        protocol: u16,
        accepted: bool,
        code: String,
    },
    FaceComponentMaskResponse {
        protocol: u16,
        request_id: String,
        accepted: bool,
        code: String,
    },
    FaceWardrobeResponse {
        protocol: u16,
        request_id: String,
        accepted: bool,
        code: String,
        report: Option<Box<Value>>,
    },
    FaceInteractionResponse {
        protocol: u16,
        accepted: bool,
        code: String,
    },
    SelectedSpeechResponse {
        protocol: u16,
        request_id: String,
        outcome: String,
        operation_id: Option<String>,
        status: Option<Box<Value>>,
        code: Option<String>,
    },
    MediaUsePlan {
        protocol: u16,
        plan_id: PlanId,
        resource_handle: ResourceHandleId,
        output_port: PortId,
    },
    WebRtcPlanReady {
        protocol: u16,
        generation: u32,
        plan_id: PlanId,
    },
    WebRtcSignal {
        protocol: u16,
        source_host_id: HostId,
        source_boot_id: BootId,
        signal: BrowserWebRtcSignal,
    },
    WebRtcGrant {
        protocol: u16,
        generation: u32,
        index: u16,
        total: u16,
        grant: Option<BrowserWebRtcGrant>,
    },
    ReturnChallenge {
        protocol: u16,
        challenge: PartReturnChallenge,
    },
    Refused {
        protocol: u16,
        code: String,
    },
}
