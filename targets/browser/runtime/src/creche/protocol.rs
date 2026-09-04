use crate::source_interaction::SourceInteractionEvidence;
use conduit_body::{Body, BodyMembership};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize)]
pub(super) struct GraduationReadiness {
    pub(super) schema: &'static str,
    pub(super) body_id: String,
    pub(super) durable_identity: bool,
    pub(super) birth_evidence: bool,
    pub(super) current_admitted_part: bool,
    pub(super) active_form_count: usize,
    pub(super) ready: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub(super) struct GraduationReceipt {
    pub(super) schema: String,
    pub(super) body_id: String,
    pub(super) sequence: u64,
    pub(super) sign_id: String,
    pub(super) choice: String,
    pub(super) patchbay_plan_id: Option<String>,
    pub(super) patchbay_implementation_id: Option<String>,
    pub(super) creche_required: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub(super) struct BirthReceipt {
    pub(super) schema: String,
    pub(super) disposition: String,
    pub(super) initial_forms: Vec<InitialFormReceipt>,
    pub(super) initial_review: super::review::InitialWorkloadReview,
    pub(super) body_id: String,
    pub(super) friendly_name: String,
    pub(super) birth_sequence: u64,
    pub(super) birth_sign_id: String,
    pub(super) state: String,
    pub(super) here_part_id: Option<String>,
    pub(super) host_id: Option<String>,
    pub(super) boot_id: Option<String>,
    pub(super) membership_revision: u64,
    pub(super) workload_revision: u64,
    pub(super) wake_id: Option<String>,
    pub(super) plan_id: Option<String>,
    pub(super) active_play_id: Option<String>,
    pub(super) source_interaction: SourceInteractionEvidence,
    pub(super) graduation: Option<GraduationReceipt>,
    pub(super) raw_body: Body,
    pub(super) raw_membership: BodyMembership,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub(super) struct InitialFormReceipt {
    pub(super) name: String,
    pub(super) source_document_id: String,
    pub(super) checked_form_id: String,
}
