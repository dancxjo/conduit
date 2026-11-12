//! Authenticated local-control request shapes; admission remains in the owner.
use super::*;

#[derive(Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "kebab-case", deny_unknown_fields)]
pub(super) enum Request {
    Status {
        protocol: u16,
        token: Vec<u8>,
    },
    BodyInspect {
        protocol: u16,
        token: Vec<u8>,
    },
    BodyInvite {
        protocol: u16,
        token: Vec<u8>,
        ttl_seconds: u64,
        candidates: Option<Vec<RendezvousCandidate>>,
    },
    BodyPrepareInvitationRoute {
        protocol: u16,
        token: Vec<u8>,
        ttl_seconds: u64,
        bind: std::net::SocketAddr,
        public_url: String,
        certificate: std::path::PathBuf,
        private_key: std::path::PathBuf,
        authorize_route: bool,
    },
    BodyAdmit {
        protocol: u16,
        token: Vec<u8>,
        request: Box<PortableSpawnAdmissionRequest>,
    },
    BodyBrowserStart {
        protocol: u16,
        token: Vec<u8>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        expected_host_id: Option<String>,
        new_host_verifying_key: Option<[u8; 32]>,
        maximum_millis: u64,
    },
    BodyBrowserBegin {
        protocol: u16,
        token: Vec<u8>,
        window_id: String,
        binding: LinkBindingId,
        frame: Box<BrowserAdmissionIngress>,
        encoded_bytes: u32,
    },
    BodyBrowserComplete {
        protocol: u16,
        token: Vec<u8>,
        window_id: String,
        frame: Box<BrowserAdmissionIngress>,
    },
    BodyBrowserOffer {
        protocol: u16,
        token: Vec<u8>,
        window_id: String,
        credential: MembershipCredential,
        disclosure: OfferDisclosureRequest,
    },
    BodyBrowserMaskRoute {
        protocol: u16,
        token: Vec<u8>,
        window_id: String,
        credential: MembershipCredential,
        binding: LinkBindingId,
        #[serde(default)]
        evidence: Option<Box<crate::durable_host::owner::BrowserCarrierLineEvidence>>,
    },
    BodyBrowserShow {
        protocol: u16,
        token: Vec<u8>,
        window_id: String,
        binding: LinkBindingId,
        request: OwnerFaceSnapshotRequest,
        show: Box<MaskShow>,
    },
    BodyBrowserComponentMask {
        protocol: u16,
        token: Vec<u8>,
        window_id: String,
        binding: LinkBindingId,
        request: OwnerFaceSnapshotRequest,
        show: Box<MaskShow>,
        look: conduit_std_host::browser_admission::BrowserComponentLook,
        recipe: conduit_std_host::browser_admission::BrowserComponentRecipe,
    },
    BodyBrowserWardrobe {
        protocol: u16,
        token: Vec<u8>,
        window_id: String,
        binding: LinkBindingId,
        request: OwnerFaceSnapshotRequest,
        owner_plan_id: Option<conduit_core::PlanId>,
        basis_revision: u64,
        action: Option<MaskWardrobeAction>,
    },
    BodyBrowserAbort {
        protocol: u16,
        token: Vec<u8>,
        window_id: String,
    },
    BodyBrowserCancel {
        protocol: u16,
        token: Vec<u8>,
        window_id: String,
    },
    BodyBrowserLeave {
        protocol: u16,
        token: Vec<u8>,
        window_id: String,
        credential: MembershipCredential,
    },
    BodyMemberCarrierReceiver {
        protocol: u16,
        token: Vec<u8>,
    },
    BodyAttestAdmissionReceipt {
        protocol: u16,
        token: Vec<u8>,
        receipt: Box<crate::durable_host::owner::MembershipReceiptDocument>,
        expected_owner: Box<conduit_body::FederationPeer>,
    },
    BodyMemberCarrierBegin {
        protocol: u16,
        token: Vec<u8>,
        credential: MembershipCredential,
        advertisement: Box<HostAdvertisement>,
        line: conduit_core::LineId,
        security: conduit_core::LineSecurity,
        expires_at_millis: u64,
        maximum_frames: u32,
    },
    BodyMemberCarrierComplete {
        protocol: u16,
        token: Vec<u8>,
        challenge: Box<conduit_body::FederationChallenge>,
        signature: Vec<u8>,
    },
    BodyMemberCarrierMaskRoute {
        protocol: u16,
        token: Vec<u8>,
        session: conduit_core::LinkBindingId,
        face_line: Box<conduit_core::LineOffer>,
        return_line: Box<conduit_core::LineOffer>,
        #[serde(default)]
        interaction_line: Option<Box<conduit_core::LineOffer>>,
    },
    BodyMemberCarrierShow {
        protocol: u16,
        token: Vec<u8>,
        session: conduit_core::LinkBindingId,
        request: OwnerFaceSnapshotRequest,
        show: Box<MaskShow>,
    },
    BodyMemberCarrierInteraction {
        protocol: u16,
        token: Vec<u8>,
        session: conduit_core::LinkBindingId,
        request: OwnerFaceSnapshotRequest,
        show: Box<MaskShow>,
        interaction: conduit_face::FaceInteraction,
    },
    BodyMemberPresenceRenew {
        protocol: u16,
        token: Vec<u8>,
        session: conduit_core::LinkBindingId,
        sequence: u64,
    },
    BodyMemberCarrierInspect {
        protocol: u16,
        token: Vec<u8>,
        session: conduit_core::LinkBindingId,
    },
    BodyMemberCarrierRetire {
        protocol: u16,
        token: Vec<u8>,
        session: conduit_core::LinkBindingId,
    },
    BodyReturn {
        protocol: u16,
        token: Vec<u8>,
        encounter: Box<crate::durable_host::MemberEncounter>,
    },
    BodyMemberReturnBegin {
        protocol: u16,
        token: Vec<u8>,
        credential: MembershipCredential,
        advertisement: Box<HostAdvertisement>,
        presentation: Option<Box<conduit_body::MemberReturnImagePresentation>>,
        images: Vec<Vec<u8>>,
    },
    /// Explicit protected recovery from the owner's durable Part/key custody.
    /// The supplied credential is a scope hint, never the transition authority.
    BodyMemberRecoverReturnBegin {
        protocol: u16,
        token: Vec<u8>,
        credential: MembershipCredential,
        advertisement: Box<HostAdvertisement>,
        presentation: Box<conduit_body::MemberReturnImagePresentation>,
        images: Vec<Vec<u8>>,
    },
    BodyMemberReturnComplete {
        protocol: u16,
        token: Vec<u8>,
        proof: Box<crate::durable_host::owner::MemberReturnProofDocument>,
    },
    BodyMemberLeave {
        protocol: u16,
        token: Vec<u8>,
        credential: MembershipCredential,
    },
    Face {
        protocol: u16,
        token: Vec<u8>,
        request: OwnerFaceSnapshotRequest,
    },
    BodyLocalFace {
        protocol: u16,
        token: Vec<u8>,
    },
    BodyLocalWardrobe {
        protocol: u16,
        token: Vec<u8>,
        body_id: conduit_body::BodyId,
        face_id: String,
        face_revision: u64,
        advertisement: conduit_core::HostAdvertisement,
        owner_plan_id: Option<conduit_core::PlanId>,
        basis_revision: u64,
        action: Option<MaskWardrobeAction>,
    },
    BodyNativeMaskRoute {
        protocol: u16,
        token: Vec<u8>,
        receipt: Box<MemberPresentationBasis>,
        face_line: Box<LineOffer>,
        return_line: Box<LineOffer>,
        #[serde(default)]
        terminal_interaction_line: Option<Box<LineOffer>>,
        #[serde(default)]
        native_interaction_line: Option<Box<LineOffer>>,
        #[serde(default)]
        spoken_selection: Option<
            Box<conduit_std_host::direct_spoken_template_mask::RemoteSpokenTemplateSelection>,
        >,
        expires_at_millis: u64,
    },
    BodyNativeComponentMask {
        protocol: u16,
        token: Vec<u8>,
        receipt: Box<MemberPresentationBasis>,
        request: OwnerFaceSnapshotRequest,
        show: Box<MaskShow>,
        look: conduit_face::ComponentMaskLook,
        recipe: conduit_std_host::browser_admission::BrowserComponentRecipe,
    },
    BodyNativeMaskShow {
        protocol: u16,
        token: Vec<u8>,
        request: OwnerFaceSnapshotRequest,
        show: Box<MaskShow>,
    },
    BirthWidgetFace {
        protocol: u16,
        token: Vec<u8>,
    },
    BirthInteraction {
        protocol: u16,
        token: Vec<u8>,
        show: Box<MaskShow>,
        interaction: FaceInteraction,
    },
    BodyInteraction {
        protocol: u16,
        token: Vec<u8>,
        show: Box<MaskShow>,
        interaction: FaceInteraction,
        #[serde(default)]
        not_after_millis: Option<u64>,
    },
    #[cfg(unix)]
    BodyAttachedTerminalInteraction {
        protocol: u16,
        token: Vec<u8>,
        route_plan_id: conduit_core::PlanId,
        show: Box<MaskShow>,
        interaction: FaceInteraction,
    },
    #[cfg(unix)]
    BodyAttachedTerminalWardrobe {
        protocol: u16,
        token: Vec<u8>,
        route_plan_id: conduit_core::PlanId,
        show: Box<MaskShow>,
        basis_revision: u64,
        command: terminal_attach::TerminalWardrobeCommand,
    },
    #[cfg(unix)]
    BodyAttachedTerminalRefreshShow {
        protocol: u16,
        token: Vec<u8>,
        route_plan_id: conduit_core::PlanId,
        show: Box<MaskShow>,
    },
    BodyBrowserInteraction {
        protocol: u16,
        token: Vec<u8>,
        window_id: String,
        binding: LinkBindingId,
        request: OwnerFaceSnapshotRequest,
        show: Box<MaskShow>,
        interaction: FaceInteraction,
        #[serde(default)]
        not_after_millis: Option<u64>,
    },
    BodyNativeGuestInteraction {
        protocol: u16,
        token: Vec<u8>,
        request: OwnerFaceSnapshotRequest,
        show: Box<MaskShow>,
        interaction: FaceInteraction,
        not_after_millis: u64,
    },
    #[cfg(target_os = "linux")]
    Notebook {
        protocol: u16,
        token: Vec<u8>,
        command: crate::cli::NotebookCommand,
    },
    BodyStart {
        protocol: u16,
        token: Vec<u8>,
        maximum_millis: u64,
        #[serde(default)]
        todo_new_list: Option<String>,
    },
    BodyLull {
        protocol: u16,
        token: Vec<u8>,
    },
    BodyExportSeed {
        protocol: u16,
        token: Vec<u8>,
        destination: PathBuf,
    },
    Join {
        protocol: u16,
        token: Vec<u8>,
        expected_boot_id: String,
        expected_offer_generation: u64,
        claim: SpawnInvitationClaim,
        secret: Vec<u8>,
    },
    InstallBodyContext {
        protocol: u16,
        token: Vec<u8>,
        expected_boot_id: String,
        expected_offer_generation: u64,
        context: BodyConversationContext,
    },
    ObserveLocalModelPool {
        protocol: u16,
        token: Vec<u8>,
        realization: PoolRealizationEnvelope,
    },
    PrepareRemote {
        protocol: u16,
        token: Vec<u8>,
        expected_boot_id: String,
        expected_offer_generation: u64,
        plan: Box<Plan>,
    },
    PreparePoolMember {
        protocol: u16,
        token: Vec<u8>,
        expected_boot_id: String,
        expected_offer_generation: u64,
        plan: Box<Plan>,
        selection: PoolSelectionEvidence,
        consumer_placement_id: PlacementId,
    },
    ExchangeRemote {
        protocol: u16,
        token: Vec<u8>,
        frame: Vec<u8>,
    },
    ReleaseRemote {
        protocol: u16,
        token: Vec<u8>,
    },
    InspectWorkerText {
        protocol: u16,
        token: Vec<u8>,
        identity: ActivePlayIdentity,
        expected_offer_generation: u64,
    },
    CompleteWorkerText {
        protocol: u16,
        token: Vec<u8>,
        result: Box<conduit_std_host::worker_text_result::WorkerTextResult>,
    },
}
