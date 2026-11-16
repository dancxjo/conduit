//! Workspace request wire contract; execution remains in the owning Root.
use super::*;
use serde::Deserialize;

#[derive(Deserialize)]
#[serde(tag = "action", deny_unknown_fields)]
pub(super) enum Request {
    Arrive {
        advertisement: HostAdvertisement,
    },
    Restore {
        evidence: Box<BodyBiographyEvidence>,
        admission: Option<AdmissionManager>,
        host_id: HostId,
        boot_id: BootId,
        advertisement: HostAdvertisement,
    },
    OpenAdmitted {
        evidence: Box<BodyBiographyEvidence>,
        admission: AdmissionManager,
        host_id: HostId,
        boot_id: BootId,
        advertisement: HostAdvertisement,
    },
    Durable,
    AcknowledgeArchives {
        head_digest: [u8; 32],
    },
    InspectInvitation {
        claim: SpawnInvitationClaim,
        now_millis: u64,
    },
    CreateInvitation {
        host_id: HostId,
        boot_id: BootId,
        secret: Vec<u8>,
        nonce: [u8; 32],
        now_millis: u64,
        expires_at_millis: u64,
    },
    PrepareBrowserSpore {
        host_id: HostId,
        boot_id: BootId,
        secret: Vec<u8>,
        nonce: [u8; 32],
        now_millis: u64,
        expires_at_millis: u64,
        image_content_digest: String,
        selection: crate::creche::BrowserConfigurationSelection,
    },
    PreparePhysicalSpore {
        host_id: HostId,
        boot_id: BootId,
        secret: Vec<u8>,
        nonce: [u8; 32],
        now_millis: u64,
        expires_at_millis: u64,
        target_id: String,
        image_content_digest: Option<String>,
        reviewed_image: Option<Vec<u8>>,
    },
    AdmitInvitation {
        host_id: HostId,
        boot_id: BootId,
        advertisement: HostAdvertisement,
        proof: ReceivedSpawnProof,
        now_millis: u64,
    },
    HostLost {
        host_id: HostId,
        boot_id: BootId,
        lost_host_id: HostId,
        lost_boot_id: BootId,
    },
    Current,
    ConversationContext,
    SelectPlot {
        plot: ResidentPlot,
    },
    LibraryView {
        host_id: HostId,
        boot_id: BootId,
        source: String,
        query: String,
        revision: u32,
        joined_lines: Vec<crate::creche::JoinedLineObservation>,
    },
    TutorialView {
        revision: u32,
        playback: conduit_tutorial_plot::TutorialPlayback,
    },
    TutorialPresenterInput {
        request_identity: String,
        face_revision: u64,
        playback: conduit_tutorial_plot::TutorialPlayback,
    },
    PresentTutorialMask {
        host_id: HostId,
        boot_id: BootId,
        revision: u64,
        playback: conduit_tutorial_plot::TutorialPlayback,
    },
    AcknowledgeTutorialMask {
        acknowledgement: crate::workspace_mask::BrowserMaskAcknowledgement,
    },
    InteractWithTutorialMask {
        interaction: crate::workspace_mask::BrowserMaskInteraction,
    },
    TutorialMaskObservation,
    TutorialMaskJourney,
    BeginTutorialMaskJourney,
    PrepareTutorialMaskAlternate,
    PrepareTutorialMaskReplacement,
    PrepareTutorialMaskRestored,
    InvitationView {
        invitation_id: String,
        body_id: String,
        body_name: String,
        expires_at_millis: u64,
        transfer_uri: String,
        revision: u32,
        clipboard_available: bool,
        share_available: bool,
    },
    InvitationQr {
        transfer_uri: String,
    },
    ChangeWorkset {
        host_id: HostId,
        boot_id: BootId,
        expected_revision: u64,
        plot: ResidentPlot,
        source: String,
        edit: WorksetEdit,
    },
    Propose {
        host_id: HostId,
        boot_id: BootId,
        source: String,
        joined_lines: Vec<crate::creche::JoinedLineObservation>,
        browser_audio_authority: bool,
    },
    Started {
        host_id: HostId,
        boot_id: BootId,
        play: BodyPlayIdentity,
        wake_at_start: Wake,
    },
    Lull {
        host_id: HostId,
        boot_id: BootId,
        terminated_play: Option<BodyPlayIdentity>,
    },
    Failed {
        host_id: HostId,
        boot_id: BootId,
        rejections: Vec<conduit_body::WakeRejectionEvidence>,
    },
    Fulfill {
        host_id: HostId,
        boot_id: BootId,
        authority_grant_id: AuthorityGrantId,
        attribution: String,
    },
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct ReceivedSpawnProof {
    pub(super) invitation_id: conduit_body::SpawnInvitationId,
    pub(super) body_id: conduit_body::BodyId,
    pub(super) host_id: HostId,
    pub(super) boot_id: BootId,
    pub(super) nonce: [u8; 32],
    pub(super) signature: Vec<u8>,
}
