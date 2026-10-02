//! Producer-owned semantic and visual evidence for the native body journey.

use super::{ConduitosError, JourneyProof};
use std::{collections::BTreeMap, path::Path};

pub(super) fn write(target: &Path, proof: &JourneyProof) -> Result<(), ConduitosError> {
    use crate::commands::body_journey_track::{
        self, ConstructionStage, ConstructionTruth, TrackIdentities, TrackSource,
    };
    let facts = vec![
        serde_json::json!({"open_effects": proof.open_effects, "host_id": proof.host_id, "boot_id": proof.boot_id}),
        serde_json::json!({"profile_id": proof.profile_id, "build_id": proof.build_id, "image_id": proof.image_id}),
        serde_json::json!({"body_id": proof.body_id, "born_sign_id": proof.born_sign_id, "part_id": proof.part_id}),
        serde_json::json!({"wake_id": proof.wake_id, "wake_sign_id": proof.wake_sign_id, "plan_id": proof.plan_id}),
        serde_json::json!({"input_sign_id": proof.input_sign_id, "result_sign_id": proof.result_sign_id, "result": proof.result}),
        serde_json::json!({"face_id": proof.inspector_presentation_id, "show_id": proof.inspector_manifestation_id, "mask": proof.native_mask, "mask_actions": proof.native_mask["mask_actions"]}),
        serde_json::json!({"workset": proof.workset, "workload_sign_id": proof.workload_sign_id, "plan_id": proof.plan_id}),
        serde_json::json!({"host_id": proof.usb_line_peer_host_id, "boot_id": proof.usb_line_peer_boot_id, "membership": proof.usb_line_membership, "line_id": proof.usb_line_id, "plan_id": proof.usb_line_plan_id}),
        serde_json::json!({"loss_sign_id": proof.loss_sign_id, "invalidated_show_id": proof.resize_invalidated_manifestation_id, "input_refused": proof.resize_input_refused_while_invalidated, "final_membership": proof.usb_line_final_membership}),
        serde_json::json!({"current_show_id": proof.resize_current_manifestation_id, "plan_id": proof.plan_id, "play_sign_id": proof.play_sign_id, "mask": proof.native_mask}),
        serde_json::json!({"remained_alive": proof.remained_alive, "active_play_id": proof.active_play_id, "result_sign_id": proof.result_sign_id}),
        serde_json::json!({"body_retained_after_lull": proof.body_retained_after_lull, "lull_sign_id": proof.lull_sign_id}),
        serde_json::json!({"fulfilled_sign_id": proof.fulfilled_sign_id, "body_fulfilled": proof.body_fulfilled}),
    ];
    body_journey_track::write(
        TrackSource {
            commit: proof.base_commit.clone(),
            track_id: "native-graphical",
            embodiment: "freestanding-native-body",
            mask_plot_id: "mask/native-graphical@1",
            construction: vec![
                ConstructionTruth {
                    host_id: proof.host_id.clone(),
                    profile: ConstructionStage::exact(proof.profile_id.clone()),
                    build: ConstructionStage::exact(proof.build_id.clone()),
                    image: ConstructionStage::exact(proof.image_id.clone()),
                },
                ConstructionTruth {
                    host_id: proof.usb_line_peer_host_id.clone(),
                    profile: ConstructionStage::omitted("The USB peer joined as an already-running Host; this producer retained no peer profile make identity."),
                    build: ConstructionStage::omitted("The USB peer joined as an already-running Host; this producer retained no peer build identity."),
                    image: ConstructionStage::omitted("The USB peer joined as an already-running Host; this producer retained no peer image identity."),
                },
            ],
            identities: TrackIdentities {
                body: proof.body_id.clone(),
                host: proof.host_id.clone(),
                boot: proof.boot_id.clone(),
                peer_host: proof.usb_line_peer_host_id.clone(),
                peer_boot: proof.usb_line_peer_boot_id.clone(),
                plan: proof.plan_id.clone(),
                distributed_plan: Some(proof.usb_line_plan_id.clone()),
                play: proof.active_play_id.clone(),
                face: proof.inspector_presentation_id.clone(),
                show: proof.inspector_manifestation_id.clone(),
                line: Some(proof.usb_line_id.clone()),
                signs: BTreeMap::from([
                    ("body.born", proof.born_sign_id.clone()),
                    ("body.awake", proof.wake_sign_id.clone()),
                    ("plot.used", proof.result_sign_id.clone()),
                    ("workload.revised", proof.workload_sign_id.clone()),
                    ("host.added", proof.play_sign_id.clone()),
                    ("fault.observed", proof.loss_sign_id.clone()),
                    ("body.repaired", proof.play_sign_id.clone()),
                    ("body.long-running", proof.result_sign_id.clone()),
                    ("body.lulled", proof.lull_sign_id.clone()),
                    ("body.fulfilled", proof.fulfilled_sign_id.clone()),
                ]),
            },
            facts,
            mask_actions: proof.native_mask["mask_actions"].clone(),
            action_events: BTreeMap::from([
                ("journey.bootstrap", "The exact ConduitOS Host boot displayed Crèche before a Body existed and accepted the bounded bootstrap action.".into()),
                ("journey.birth", "Crèche created the native Body and its first admitted wake; the next frame showed that Body's home.".into()),
                ("journey.useful-work", "Keyboard input traversed the standing Plot and produced the retained HELLO result.".into()),
                ("journey.break-recover", "USB Line loss remained lost; a separately invalidated native Show was rebuilt and became usable later in the same run.".into()),
                ("journey.rest-finish", "The native Body was explicitly lulled, retained, and then irreversibly fulfilled.".into()),
            ]),
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
                    "plot.used",
                    "input-continued",
                    "Typing hello produces HELLO through the running keyboard plot.",
                ),
                (
                    "body.inspected",
                    "patchbay-current-canvas",
                    "The native body's resident Patchbay inspects its current keyboard plot.",
                ),
                (
                    "workload.revised",
                    "workload-revised",
                    "A plot is admitted into the living body's workset.",
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
                    "The running plot accepts another input in the same session.",
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
