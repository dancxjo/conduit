use crate::book_runner::interaction::SourceInteractionEvidence;
use conduit_body::{Body, BodyMembership};
use serde::Serialize;

#[derive(Clone, Debug, Serialize)]
pub(super) struct GraduationReadiness {
    pub(super) schema: &'static str,
    pub(super) body_id: String,
    pub(super) durable_identity: bool,
    pub(super) birth_evidence: bool,
    pub(super) current_admitted_part: bool,
    pub(super) intended_program: String,
    pub(super) ready: bool,
}

#[derive(Clone, Debug, Serialize)]
pub(super) struct GraduationReceipt {
    pub(super) schema: &'static str,
    pub(super) body_id: String,
    pub(super) sequence: u64,
    pub(super) sign_id: String,
    pub(super) choice: &'static str,
    pub(super) patchbay_plan_id: Option<String>,
    pub(super) patchbay_implementation_id: Option<String>,
    pub(super) creche_required: bool,
}

#[derive(Clone, Debug, Serialize)]
pub(super) struct BirthReceipt {
    pub(super) schema: &'static str,
    pub(super) disposition: &'static str,
    pub(super) source_document_id: String,
    pub(super) checked_form_id: String,
    pub(super) seed_id: String,
    pub(super) body_id: String,
    pub(super) friendly_name: String,
    pub(super) initial_program: String,
    pub(super) birth_sequence: u64,
    pub(super) birth_sign_id: String,
    pub(super) state: &'static str,
    pub(super) here_part_id: Option<String>,
    pub(super) host_id: Option<String>,
    pub(super) boot_id: Option<String>,
    pub(super) membership_revision: u64,
    pub(super) wake_id: Option<String>,
    pub(super) plan_id: Option<String>,
    pub(super) active_play_id: Option<String>,
    pub(super) source_interaction: SourceInteractionEvidence,
    pub(super) graduation: Option<GraduationReceipt>,
    pub(super) raw_body: Body,
    pub(super) raw_membership: BodyMembership,
}
