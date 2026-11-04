//! Local-control reply shapes; serialization grants no authority.
use super::*;

#[derive(Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "kebab-case")]
pub(super) enum Response {
    Status {
        protocol: u16,
        target_id: String,
        image_content_digest: String,
        advertisement: HostAdvertisement,
    },
    BodyTruth {
        protocol: u16,
        truth: serde_json::Value,
    },
    BodyInvitation {
        protocol: u16,
        invitation: Box<conduit_body::PortableInvitation>,
    },
    BodyInvitationRoutePrepared {
        protocol: u16,
        candidate: RendezvousCandidate,
    },
    BodyAdmitted {
        protocol: u16,
        receipt: Box<conduit_body::PortableAdmissionReceipt>,
    },
    BodyBrowserWindow {
        protocol: u16,
        window_id: String,
        url: String,
        body_id: conduit_body::BodyId,
        maximum_millis: u64,
    },
    BodyBrowserChallenge {
        protocol: u16,
        frame: Box<BrowserAdmissionEgress>,
    },
    BodyBrowserSnapshot {
        protocol: u16,
        snapshot: Box<crate::durable_host::owner::BrowserAdmittedSnapshot>,
    },
    BodyBrowserOffer {
        protocol: u16,
        offer: Box<HostOfferProjection>,
    },
    BodyBrowserMaskRoute {
        protocol: u16,
        route: Box<RemoteOwnerMaskRouteSeal>,
    },
    BodyBrowserShowAccepted {
        protocol: u16,
    },
    BodyBrowserComponentMaskAccepted {
        protocol: u16,
    },
    BodyBrowserWardrobeReport {
        protocol: u16,
        report: Box<serde_json::Value>,
    },
    BodyBrowserAborted {
        protocol: u16,
    },
    BodyBrowserCancelled {
        protocol: u16,
    },
    BodyBrowserLeft {
        protocol: u16,
        biography: Box<BodyBiographyEvidence>,
    },
    BodyMemberCarrierReceiver {
        protocol: u16,
        receiver: Box<conduit_body::FederationPeer>,
    },
    BodyAttestedAdmissionReceipt {
        protocol: u16,
        receipt: Box<conduit_body::SignedPortableAdmissionReceipt>,
    },
    BodyMemberCarrierChallenge {
        protocol: u16,
        challenge: Box<conduit_body::FederationChallenge>,
        signature: Vec<u8>,
    },
    BodyMemberPresenceRenewed {
        protocol: u16,
        lease: Box<conduit_body::HostPresenceLease>,
    },
    BodyMemberCarrierInspection {
        protocol: u16,
        inspection: Box<conduit_body::FederationInspection>,
    },
    BodyMemberCarrierRetired {
        protocol: u16,
    },
    BodyMemberReturnChallenge {
        protocol: u16,
        challenge: Box<conduit_body::PartReturnChallenge>,
    },
    BodyMemberReturned {
        protocol: u16,
        credential: MembershipCredential,
    },
    BodyMemberLeft {
        protocol: u16,
    },
    Face {
        protocol: u16,
        response: Box<OwnerFaceSnapshotResponse>,
    },
    BodyLocalFace {
        protocol: u16,
        face: Box<Face>,
        advertisement: HostAdvertisement,
    },
    BodyLocalWardrobe {
        protocol: u16,
        report: Box<serde_json::Value>,
        face: Option<Box<Face>>,
        reading_refusal: Option<String>,
    },
    BodyNativeMaskRoute {
        protocol: u16,
        route: Box<RemoteOwnerMaskRouteSeal>,
    },
    BodyNativeMaskShowAccepted {
        protocol: u16,
    },
    BirthWidgetFace {
        protocol: u16,
        face: Box<Face>,
        advertisement: HostAdvertisement,
    },
    BirthChanged {
        protocol: u16,
        face: Box<Face>,
    },
    BirthCompleted {
        protocol: u16,
        body_id: conduit_body::BodyId,
        face: Box<Face>,
    },
    BodyInteraction {
        protocol: u16,
        result: Box<serde_json::Value>,
    },
    #[cfg(unix)]
    BodyAttachedTerminalWardrobe {
        protocol: u16,
        report: Box<serde_json::Value>,
    },
    #[cfg(unix)]
    BodyAttachedTerminalRefreshedShow {
        protocol: u16,
        show: Box<MaskShow>,
        advertisement: HostAdvertisement,
    },
    BodyRunRequested {
        protocol: u16,
        maximum_millis: u64,
    },
    BodySeedExported {
        protocol: u16,
        receipt: conduit_host_make::DeploymentRealizationReceipt,
    },
    BodyLullRequested {
        protocol: u16,
        active_play_id: Option<String>,
    },
    Join {
        protocol: u16,
        advertisement: HostAdvertisement,
        invitation_id: String,
        body_id: String,
        nonce: [u8; 32],
        signature: Vec<u8>,
        observed_at_millis: u64,
    },
    BodyContextInstalled {
        protocol: u16,
        advertisement: HostAdvertisement,
    },
    LocalModelPoolObserved {
        protocol: u16,
        observation: PoolRealizationObservation,
    },
    RemotePrepared {
        protocol: u16,
        identity: ActivePlayIdentity,
        hello_frames: Vec<Vec<u8>>,
    },
    RemoteExchanged {
        protocol: u16,
        responses: Vec<Vec<u8>>,
        active: bool,
    },
    RemoteReleased {
        protocol: u16,
    },
    WorkerText {
        protocol: u16,
        result: Option<Box<conduit_std_host::worker_text_result::WorkerTextResult>>,
    },
    Refused {
        protocol: u16,
        code: String,
    },
}
