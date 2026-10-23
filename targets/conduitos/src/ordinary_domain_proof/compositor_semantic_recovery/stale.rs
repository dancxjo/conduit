//! Real stale-Show submission and completed framebuffer notice; capture holds
//! the frame until an actual PS/2 transition acknowledges the host checkpoint.
use super::*;
use crate::{native_mask_play::NativeMaskPlayError, sign_format::FixedText};
use conduit_face::FaceInteractionRefusal;
use core::fmt::Write;

pub(super) fn capture_refusal(
    mask: &mut NativeFaceMask,
    face: &Face,
    show: &MaskShow,
    held: &FaceInteraction,
    draft: &BirthDraft,
    target: &mut Target,
) {
    // Real input adapter initialization precedes the checkpoint, so it cannot
    // flush the host's later acknowledgement or fabricate a semantic action.
    let (mut input, _) =
        arch::Ps2Input::initialize().unwrap_or_else(|_| refuse("stale-capture-keyboard"));
    let revision = draft.revision();
    let publication = mask.publication().cloned();
    let writes = target.writes;
    if held.face_id != face.identity.as_str()
        || held.face_revision != face.revision
        || held.show_id == show.show_id.as_str()
        || mask.submit_with_display(held.clone(), target)
            != Err(NativeFaceMaskError::Mask(NativeMaskPlayError::Interaction(
                FaceInteractionRefusal::StaleShow,
            )))
        || mask.show() != Some(show)
        || mask.scene().map(|scene| scene.face()) != Some(face)
        || mask.publication().cloned() != publication
        || draft.revision() != revision
        || mask.route_keyboard() != Ok(true)
        || target.writes <= writes
        || !mask
            .scene()
            .unwrap_or_else(|| refuse("stale-scene"))
            .frame()
            .unwrap_or_else(|_| refuse("stale-frame"))
            .scene
            .commands()
            .iter()
            .any(|command| command.payload() == "Stale: refresh")
    {
        refuse("stale-submission-custody");
    }
    let format = target.format();
    let mut out = FixedText::new();
    // These actual canonical identities are JSON-escaped by the serializer;
    // their finite bounds already belong to the Face/Show contracts.
    let face_id = serde_json::to_string(&face.identity).unwrap_or_else(|_| refuse("stale-face-id"));
    let old_show =
        serde_json::to_string(&held.show_id).unwrap_or_else(|_| refuse("stale-old-show-id"));
    let current_show =
        serde_json::to_string(&show.show_id).unwrap_or_else(|_| refuse("stale-show-id"));
    let mask_plan = serde_json::to_string(&show.planned_mask.plan.plan_id)
        .unwrap_or_else(|_| refuse("stale-mask-plan-id"));
    let active_play = serde_json::to_string(&show.show.active_play_id)
        .unwrap_or_else(|_| refuse("stale-active-play-id"));
    writeln!(out,
        "CONDUIT_COMPOSITOR_STALE_REFUSAL {{\"schema\":\"conduit.conduitos/stale-show-framebuffer-fixture@1\",\"refusal\":\"StaleShow\",\"notice\":\"Stale: refresh\",\"face_id\":{},\"face_revision\":{},\"old_show_id\":{},\"current_show_id\":{},\"same_face\":true,\"same_draft\":true,\"current_show_preserved\":true,\"keyboard_available\":true,\"owner_actions\":0,\"pixels_written\":{},\"width\":{},\"height\":{},\"mask_plan_id\":{},\"active_play_id\":{},\"fixture\":true,\"physical_scanout\":false}}",
        face_id, face.revision, old_show, current_show, target.writes - writes,
        format.width, format.height, mask_plan, active_play,
    ).unwrap_or_else(|_| refuse("stale-receipt-capacity"));
    arch::early_write(out.as_bytes());
    // The host captures the completed frame before sending this real key.
    // It is checkpoint control only, never dispatched to the source owner.
    let key = input
        .receive_keyboard()
        .unwrap_or_else(|_| refuse("stale-capture-ack"));
    if key.usage() != 40 || !key.pressed() || key.modifiers() != 0 {
        refuse("stale-capture-ack-value");
    }
    arch::early_write(b"CONDUIT_COMPOSITOR_STALE_CAPTURE_ACK\n");
}
