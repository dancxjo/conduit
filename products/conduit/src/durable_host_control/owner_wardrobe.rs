//! Local authenticated view and revision-bound control of the installed owner wardrobe.

use super::{
    speech_route::{call, SpeechReply, SpeechRequest},
    PROTOCOL,
};
use conduit_core::PlanId;
use conduit_presentation::MaskWardrobeAction;
use serde_json::Value;
use std::path::Path;

pub(crate) fn report(
    state_dir: &Path,
    owner_plan_id: Option<PlanId>,
    basis_revision: u64,
    action: Option<MaskWardrobeAction>,
) -> Result<Value, String> {
    match call(state_dir, |token| SpeechRequest::OwnerWardrobe {
        protocol: PROTOCOL,
        token,
        owner_plan_id,
        basis_revision,
        action,
    })? {
        SpeechReply::OwnerWardrobe {
            protocol: PROTOCOL,
            report,
        } => Ok(*report),
        SpeechReply::Refused { code, .. } => Err(code),
        _ => Err(super::CONTROL_OUTCOME_UNKNOWN.into()),
    }
}
