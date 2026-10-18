//! Bounded observations of the real native Mask and lifecycle owner boundary.
#[cfg(not(test))]
use crate::arch;
use crate::product_journey::JourneyProjection;
use conduit_face::{Face, FaceInteraction, MaskShow};
use serde_json::{Value, json};

fn emit(prefix: &[u8], record: &Value) -> Result<(), &'static str> {
    let bytes = serde_json::to_vec(record).map_err(|_| "native-action-evidence-encoding")?;
    if bytes.len() > 96 * 1024 {
        return Err("native-action-evidence-bound");
    }
    #[cfg(not(test))]
    {
        arch::early_write(prefix);
        arch::early_write(&bytes);
        arch::early_write(b"\n");
    }
    #[cfg(test)]
    std::println!(
        "{}{}",
        core::str::from_utf8(prefix).unwrap(),
        alloc::string::String::from_utf8_lossy(&bytes)
    );
    Ok(())
}

pub(super) fn face(
    face: &Face,
    show: &MaskShow,
    observation: u64,
    play: u64,
) -> Result<(), &'static str> {
    emit(
        b"CONDUIT_NATIVE_FACE ",
        &json!({
            "schema": "conduit.native-face/v1", "face": face,
            "show": {
                "show_id": show.show_id, "mask_plot": show.mask_plot,
                "mask_plan_id": show.planned_mask.plan.plan_id,
                "mask_active_play_id": show.show.active_play_id,
                "face_id": show.face_id, "face_revision": show.face_revision,
                "manifestation_id": show.show.manifestation_id,
                "lifecycle": show.show.lifecycle,
            "host_id": show.show.host_id, "boot_id": show.show.boot_id,
            "offer_generation": show.show.offer_generation,
            "manifestation": show.show,
            },
            "observation_sequence": observation, "play_sequence": play,
        }),
    )
}

/// Observation after the source owner accepts an edit and the new Mask Show
/// acknowledges its fresh Face. This record grants no action authority.
pub(super) fn creche_change(
    before: &Face,
    prior_show: &MaskShow,
    interaction: &FaceInteraction,
    after: &Face,
    fresh_show: &MaskShow,
) -> Result<(), &'static str> {
    emit(
        b"CONDUIT_CRECHE_CHANGE ",
        &json!({
            "schema": "conduit.creche-change/v1",
            "disposition": "source-accepted-and-shown",
            "before_face_id": before.identity,
            "before_revision": before.revision,
            "before_show_id": prior_show.show_id,
            "interaction": interaction,
            "after_face_id": after.identity,
            "after_revision": after.revision,
            "after_show_id": fresh_show.show_id,
        }),
    )
}

fn snapshot(p: &JourneyProjection) -> Value {
    json!({
        "status": p.status.as_str(), "revision": p.revision,
        "source_document_id": p.source_document_id, "checked_plot_id": p.checked_plot_id,
        "expanded_plot_id": p.expanded_plot_id, "host_id": p.host_id, "boot_id": p.boot_id,
        "offer_generation": p.offer_generation, "body_id": p.body_id,
        "friendly_name": p.friendly_name, "born_sign_id": p.born_sign_id,
        "fulfilled_sign_id": p.fulfilled_sign_id, "workload_revision": p.workload_revision,
        "workload_sign_id": p.workload_sign_id, "lull_sign_id": p.lull_sign_id,
        "workload_capacity_available": p.workload_capacity_available, "part_id": p.part_id,
        "wake_id": p.wake_id, "wake_sign_id": p.wake_sign_id, "plan_id": p.plan_id,
        "partition_plan_id": p.partition_plan_id, "plan_sign_id": p.plan_sign_id,
        "active_play_id": p.active_play_id, "play_sign_id": p.play_sign_id,
        "gear_ids": p.gear_ids, "port_ids": p.port_ids, "cord_ids": p.cord_ids,
        "input_sign_id": p.input_sign_id, "loss_kind": p.loss_kind.map(|kind| kind.as_str()),
        "loss_sign_id": p.loss_sign_id, "result_sign_id": p.result_sign_id, "result": p.result,
        "text_editor_active": p.text_editor_active, "text_editor_label": p.text_editor_label,
        "result_omitted_bytes": p.result_omitted_bytes, "input_count": p.input_count,
        "kernel_sign_gap": p.kernel_sign_gap.map(|gap| json!({
            "first_sequence": gap.first_sequence, "last_sequence": gap.last_sequence, "entries": gap.entries,
        })), "last_request_id": p.last_request_id, "mask": p.mask,
    })
}

#[allow(clippy::too_many_arguments)]
pub(super) fn action(
    face: &Face,
    show: &MaskShow,
    interaction: Option<&FaceInteraction>,
    intent: &str,
    before: &JourneyProjection,
    after: &JourneyProjection,
    refusal: Option<&str>,
) -> Result<(), &'static str> {
    let prefix = if before.body_id.is_none() && after.body_id.is_some() {
        b"CONDUIT_NATIVE_BIRTH ".as_slice()
    } else {
        b"CONDUIT_NATIVE_ACTION ".as_slice()
    };
    emit(
        prefix,
        &json!({
            "schema": "conduit.native-action/v1", "interaction": interaction,
            "show_id": show.show_id, "face_id": face.identity, "face_revision": face.revision,
            "requested_intent": intent,
            "disposition": match refusal {
                Some(reason) => json!({"Refused": {"reason": reason}}),
                None => json!({"Accepted": {"operation_request_id": after.last_request_id}}),
            },
            "accepted_effects": u8::from(refusal.is_none()),
            "before": snapshot(before), "after": snapshot(after),
        }),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn snapshot_carries_actual_planned_editor_state_and_label() {
        let (mut journey, _) = crate::product_front_door::workspace_input::tests::listening();
        let resident =
            crate::native_workset::resident(crate::native_workset::NativePlot::MemoryLantern)
                .unwrap();
        journey.select_plot(&resident, journey.revision()).unwrap();
        let projection = journey.projection();
        assert!(projection.text_editor_active);
        assert_eq!(projection.text_editor_label.as_deref(), Some("Message"));
        let record = snapshot(&projection);
        assert_eq!(record["text_editor_active"], projection.text_editor_active);
        assert_eq!(
            record["text_editor_label"],
            json!(projection.text_editor_label)
        );
        let (ids, offer, _) = crate::product_journey::test_support::fixture();
        crate::product_journey::test_support::invoke(
            &mut journey,
            patchbay_control::PatchbayAction::Stop,
            &ids,
            &offer,
        )
        .unwrap();
        let stopped = journey.projection();
        assert!(!stopped.text_editor_active);
        assert_eq!(snapshot(&stopped)["text_editor_active"], false);
        assert_eq!(
            snapshot(&stopped)["text_editor_label"],
            json!(stopped.text_editor_label)
        );
    }
}
