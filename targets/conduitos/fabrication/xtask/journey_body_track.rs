//! Producer-owned semantic and visual evidence for the native body journey.

use super::{ConduitosError, JourneyProof};
use std::path::Path;

pub(super) fn write(target: &Path, proof: &JourneyProof) -> Result<(), ConduitosError> {
    use crate::commands::body_journey_track::{self, TrackIdentities, TrackSource};
    let facts = [
        serde_json::json!({"open_effects": proof.open_effects, "host_id": proof.host_id, "boot_id": proof.boot_id}),
        serde_json::json!({"profile_id": proof.profile_id, "build_id": proof.build_id, "image_id": proof.image_id}),
        serde_json::json!({"body_id": proof.body_id, "born_sign_id": proof.born_sign_id, "part_id": proof.part_id}),
        serde_json::json!({"wake_id": proof.wake_id, "wake_sign_id": proof.wake_sign_id, "plan_id": proof.plan_id}),
        serde_json::json!({"input_sign_id": proof.input_sign_id, "result_sign_id": proof.result_sign_id, "result": proof.result}),
        serde_json::json!({"presentation_id": proof.inspector_presentation_id, "manifestation_id": proof.inspector_manifestation_id}),
        serde_json::json!({"workset": proof.workset, "workload_sign_id": proof.workload_sign_id, "plan_id": proof.plan_id}),
        serde_json::json!({"host_id": proof.usb_line_peer_host_id, "boot_id": proof.usb_line_peer_boot_id, "membership": proof.usb_line_membership, "line_id": proof.usb_line_id, "plan_id": proof.usb_line_plan_id}),
        serde_json::json!({"loss_sign_id": proof.loss_sign_id, "invalidated_manifestation_id": proof.resize_invalidated_manifestation_id, "input_refused": proof.resize_input_refused_while_invalidated, "final_membership": proof.usb_line_final_membership}),
        serde_json::json!({"current_manifestation_id": proof.resize_current_manifestation_id, "plan_id": proof.plan_id, "play_sign_id": proof.play_sign_id}),
        serde_json::json!({"remained_alive": proof.remained_alive, "active_play_id": proof.active_play_id, "result_sign_id": proof.result_sign_id}),
        serde_json::json!({"body_retained_after_lull": proof.body_retained_after_lull, "lull_sign_id": proof.lull_sign_id}),
        serde_json::json!({"fulfilled_sign_id": proof.fulfilled_sign_id, "body_fulfilled": proof.body_fulfilled}),
    ];
    body_journey_track::write(
        TrackSource {
            commit: proof.base_commit.clone(),
            track_id: "native-graphical",
            embodiment: "freestanding-native-body",
            presenter_id: "presenter/native-graphical@1",
            identities: TrackIdentities {
                body: proof.body_id.clone(),
                host: proof.host_id.clone(),
                boot: proof.boot_id.clone(),
                peer_host: proof.usb_line_peer_host_id.clone(),
                peer_boot: proof.usb_line_peer_boot_id.clone(),
                plan: proof.plan_id.clone(),
                distributed_plan: Some(proof.usb_line_plan_id.clone()),
                play: proof.active_play_id.clone(),
                presentation: proof.inspector_presentation_id.clone(),
                manifestation: proof.inspector_manifestation_id.clone(),
                line: Some(proof.usb_line_id.clone()),
                signs: [
                    None,
                    None,
                    Some(proof.born_sign_id.clone()),
                    Some(proof.wake_sign_id.clone()),
                    Some(proof.result_sign_id.clone()),
                    None,
                    Some(proof.workload_sign_id.clone()),
                    Some(proof.play_sign_id.clone()),
                    Some(proof.loss_sign_id.clone()),
                    Some(proof.play_sign_id.clone()),
                    Some(proof.result_sign_id.clone()),
                    Some(proof.lull_sign_id.clone()),
                    Some(proof.fulfilled_sign_id.clone()),
                ],
            },
            facts,
        },
        &target.join("body-journey-track"),
    )
    .and_then(|()| {
        body_journey_track::attach_screenshots(
            &target.join("body-journey-track"),
            &target.join("journey-frames"),
            &[
                (
                    "body.absent",
                    "front-door-ready",
                    "ConduitOS boots to Crèche before a body is born.",
                ),
                (
                    "bootstrap.started",
                    "front-door-ready",
                    "The booted host offers Crèche, ready to create a body.",
                ),
                (
                    "body.born",
                    "body-awake",
                    "First visible frame after Birth and Wake: the new body's native home. This entrance performs both actions before this capture.",
                ),
                (
                    "body.awake",
                    "body-awake",
                    "The newly born body wakes in the native graphical shell.",
                ),
                (
                    "form.used",
                    "input-continued",
                    "Typing hello produces HELLO through the running keyboard form.",
                ),
                (
                    "body.inspected",
                    "patchbay-current-canvas",
                    "The native body's resident Patchbay inspects its current keyboard form.",
                ),
                (
                    "workload.revised",
                    "workload-revised",
                    "A form is admitted into the living body's workset.",
                ),
                (
                    "host.added",
                    "peer-attached",
                    "The second host joins through the USB Line.",
                ),
                (
                    "fault.observed",
                    "line-lost",
                    "Disconnecting the peer makes Line loss visible.",
                ),
                (
                    "body.repaired",
                    "inspector-focused",
                    "Later in this same run, the native inspector is usable after its presentation was invalidated and rebuilt. This is presentation recovery, not reconnection of the lost peer.",
                ),
                (
                    "body.long-running",
                    "input-continued",
                    "The running form accepts another input in the same session.",
                ),
                ("body.lulled", "lulled", "The body is lulled and retained."),
                (
                    "body.fulfilled",
                    "fulfilled",
                    "The native body is fulfilled and its biography closes.",
                ),
            ],
        )
    })
    .map_err(|error| ConduitosError::refusal("product-journey-track-unavailable", error))
}
