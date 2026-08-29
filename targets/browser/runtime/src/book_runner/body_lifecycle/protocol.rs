use crate::book_runner::interaction::SourceInteractionEvidence;
use conduit_body::{Body, BodyMembership};
use serde::Serialize;

#[derive(Clone, Debug, Serialize)]
pub(super) struct BirthReceipt {
    pub(super) schema: &'static str,
    pub(super) disposition: &'static str,
    pub(super) source_document_id: String,
    pub(super) checked_form_id: String,
    pub(super) seed_id: String,
    pub(super) body_id: String,
    pub(super) birth_sequence: u64,
    pub(super) birth_sign_id: String,
    pub(super) state: &'static str,
    pub(super) here_part_id: String,
    pub(super) host_id: String,
    pub(super) boot_id: String,
    pub(super) membership_revision: u64,
    pub(super) wake_id: Option<String>,
    pub(super) plan_id: Option<String>,
    pub(super) active_play_id: Option<String>,
    pub(super) source_interaction: SourceInteractionEvidence,
    pub(super) raw_body: Body,
    pub(super) raw_membership: BodyMembership,
}
