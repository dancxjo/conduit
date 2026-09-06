//! Bounded retained evidence for the Host fabrication capstone.

use conduit_body::{BodyMembership, PartId};
use conduit_core::{ArtifactId, Plan, SignId};
use conduit_host_fabrication::{BuildManifest, HostImage, ImageBootIdentity};
use conduit_presentation::{ManifestationSet, Presentation};
use serde::Serialize;

pub(super) const SCHEMA: &str = "conduit.host/multi-profile-body-capstone@1";
pub(super) const MAX_CAPSTONE_RECEIPT_BYTES: usize = 512 * 1024;

#[derive(Debug, Serialize)]
pub struct ImageEvidence {
    pub profile_name: String,
    pub manifest: BuildManifest,
    pub image: HostImage,
    pub artifact_id: ArtifactId,
    pub encoded_bytes: usize,
}

#[derive(Debug, Serialize)]
pub struct UpdateEvidence {
    pub source: &'static str,
    pub interaction_manifestation_id: String,
    pub semantic_subject: String,
    pub semantic_action: String,
    pub sign_id: SignId,
    pub prior_presentation_id: String,
    pub revised_presentation_id: String,
    pub native_manifestation_id: String,
    pub browser_manifestation_id: String,
}

#[derive(Debug, Serialize)]
pub struct RefusalEvidence {
    pub missing_live_presenter: bool,
    pub headless_graphical_placement: bool,
    pub stale_boot: bool,
    pub stale_generation: bool,
    pub cross_wired_manifestation: bool,
}

#[derive(Debug, Serialize)]
pub struct CapstoneReceipt {
    pub schema: &'static str,
    pub images: Vec<ImageEvidence>,
    pub boots: Vec<ImageBootIdentity>,
    pub membership: BodyMembership,
    pub part_ids: Vec<PartId>,
    pub plan: Plan,
    pub initial_presentation: Presentation,
    pub initial_manifestations: ManifestationSet,
    pub replaced_manifestations: ManifestationSet,
    pub revised_presentation: Presentation,
    pub revised_manifestations: ManifestationSet,
    pub update: UpdateEvidence,
    pub refusals: RefusalEvidence,
}

pub(super) struct BuiltProfile {
    pub name: &'static str,
    pub image: HostImage,
    pub bytes: Vec<u8>,
}
