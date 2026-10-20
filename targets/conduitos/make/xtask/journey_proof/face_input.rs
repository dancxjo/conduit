//! Real keyboard negative exercise; no fixture callback or alternate ingress.
use super::super::{qemu_artifacts::Artifacts, qmp};
use super::{face_correspondence, hid_qmp, io_error, journey_input, stage_count, ConduitosError};
use std::{fs, os::unix::net::UnixStream, path::Path, process::Child};

pub(super) fn unavailable_wake(
    serial_path: &Path,
    child: &mut Child,
    qmp: &mut UnixStream,
    reader: &mut qmp::Reader,
    artifacts: &mut Artifacts,
) -> Result<(), ConduitosError> {
    let before = face_correspondence::refused_wake_count(
        &fs::read_to_string(serial_path).map_err(io_error)?,
    )?;
    let handled_before = stage_count(serial_path, "CONDUIT_PRODUCT_ACTION handled wake")?;
    journey_input::key_pair(qmp, reader, "f4", "journey-unavailable-wake")?;
    journey_input::wait_for_record(
        serial_path,
        child,
        "journey-unavailable-wake-timeout",
        "unavailable-wake",
        |serial| Ok(face_correspondence::refused_wake_count(serial)? > before),
    )?;
    hid_qmp::wait_for_stage_count(
        serial_path,
        child,
        "CONDUIT_PRODUCT_ACTION handled wake",
        handled_before + 1,
        "journey-unavailable-wake-restore-timeout",
    )?;
    // A refused action can restore the same pixels. Its receipt,
    // rather than a changed image, establishes the refusal.
    artifacts.capture(qmp, reader, "wake-refused", false)?;
    Ok(())
}

pub(super) fn return_and_reenter(
    serial_path: &Path,
    child: &mut Child,
    qmp: &mut UnixStream,
    reader: &mut qmp::Reader,
    artifacts: &mut Artifacts,
) -> Result<(), ConduitosError> {
    // Leave the Face, return Home, and reenter the same resident after Stop.
    journey_input::key_pair(qmp, reader, "esc", "journey-return-application")?;
    let home_before = stage_count(serial_path, "CONDUIT_HOME_CHECKPOINT returned")?;
    journey_input::key_pair(qmp, reader, "esc", "journey-return-home")?;
    hid_qmp::wait_for_stage_count(
        serial_path,
        child,
        "CONDUIT_HOME_CHECKPOINT returned",
        home_before + 1,
        "journey-return-home-timeout",
    )?;
    artifacts.capture(qmp, reader, "home-returned", true)?;
    journey_input::key_pair(qmp, reader, "tab", "journey-reselect-patchbay")?;
    let patchbay_before = stage_count(serial_path, "CONDUIT_HOME_CHECKPOINT patchbay-opened")?;
    journey_input::key_pair(qmp, reader, "ret", "journey-reenter-patchbay")?;
    hid_qmp::wait_for_stage_count(
        serial_path,
        child,
        "CONDUIT_HOME_CHECKPOINT patchbay-opened",
        patchbay_before + 1,
        "journey-reenter-patchbay-timeout",
    )?;
    artifacts.capture(qmp, reader, "patchbay-reentered", true)?;
    Ok(())
}

/// Retain the real old-Mask Enter after an owned modifier release changes the
/// owner. Both transitions share one USB report; there is no replay ingress.
pub(super) fn stale_modifier_release(
    serial_path: &Path,
    child: &mut Child,
    stream: &mut UnixStream,
    reader: &mut qmp::Reader,
    artifacts: &mut Artifacts,
) -> Result<(), ConduitosError> {
    let journey_before = stage_count(serial_path, "CONDUIT_PRODUCT_JOURNEY ")?;
    hid_qmp::send_named_keys(stream, reader, &["shift"], true, "journey-hold-owner-shift")?;
    hid_qmp::wait_for_stage_count(
        serial_path,
        child,
        "CONDUIT_PRODUCT_JOURNEY ",
        journey_before + 1,
        "journey-held-shift-timeout",
    )?;
    // F1 explicitly relinquishes the automatic editor surface before its
    // ordinary application shortcut is considered. F2 then enters a normal
    // inspector; its second press opens Details and clears local focus.
    journey_input::key_pair(stream, reader, "f1", "journey-leave-auto-editor")?;
    let shown_before = stage_count(serial_path, "CONDUIT_WORKSPACE_FACE shown")?;
    journey_input::key_pair(stream, reader, "f2", "journey-enter-ordinary-inspector")?;
    hid_qmp::wait_for_stage_count(
        serial_path,
        child,
        "CONDUIT_WORKSPACE_FACE shown",
        shown_before + 1,
        "journey-ordinary-inspector-timeout",
    )?;
    let redraw_before = stage_count(serial_path, "CONDUIT_PRODUCT_JOURNEY ")?;
    journey_input::key_pair(stream, reader, "f2", "journey-open-inspector-details")?;
    hid_qmp::wait_for_stage_count(
        serial_path,
        child,
        "CONDUIT_PRODUCT_JOURNEY ",
        redraw_before + 1,
        "journey-inspector-details-timeout",
    )?;
    let serial = fs::read_to_string(serial_path).map_err(io_error)?;
    let shown = face_correspondence::records(&serial, "CONDUIT_NATIVE_FACE ")?
        .pop()
        .ok_or_else(|| super::refusal("stale exercise has no actual native Face"))?;
    let face: conduit_face::Face = serde_json::from_value(shown["face"].clone())
        .map_err(|error| super::refusal(error.to_string()))?;
    // Details emits action controls in source utterance order. Reverse Tab
    // from its cleared focus chooses the final available control; establish
    // that actual source contract rather than guessing a fixed tab count.
    let last = face
        .actions
        .iter()
        .rev()
        .find(|action| action.availability.is_available())
        .ok_or_else(|| super::refusal("stale exercise has no available control"))?;
    if last.intent != "conduit.intent/stop@1" || !last.arguments.is_empty() {
        return Err(super::refusal(
            "last actual available Details control is not argument-free Stop",
        ));
    }
    let redraw_before = stage_count(serial_path, "CONDUIT_PRODUCT_JOURNEY ")?;
    journey_input::key_pair(stream, reader, "tab", "journey-shift-tab-stop")?;
    hid_qmp::wait_for_stage_count(
        serial_path,
        child,
        "CONDUIT_PRODUCT_JOURNEY ",
        redraw_before + 1,
        "journey-stop-focus-timeout",
    )?;
    let actions_before = face_correspondence::records(&serial, "CONDUIT_NATIVE_ACTION ")?.len();
    let command = serde_json::json!({"execute":"input-send-event","arguments":{"events":[
        {"type":"key","data":{"down":false,"key":{"type":"qcode","data":"shift"}}},
        {"type":"key","data":{"down":true,"key":{"type":"qcode","data":"ret"}}}
    ]}});
    qmp::request(
        stream,
        reader,
        &serde_json::to_vec(&command).expect("QMP key report is serializable"),
        "journey-release-shift-enter",
    )?;
    let observed = journey_input::wait_for_record(
        serial_path,
        child,
        "journey-stale-face-timeout",
        "real stale Face refusal",
        |text| {
            let actions = face_correspondence::records(text, "CONDUIT_NATIVE_ACTION ")?;
            Ok(actions.iter().skip(actions_before).any(|action| {
                action["disposition"]["Refused"]["reason"] == "StaleFace"
                    && action["show_id"] == shown["show"]["show_id"]
                    && action["face_id"] == shown["face"]["identity"]
                    && action["interaction"]["action_id"] == last.identity.as_str()
                    && action["requested_intent"] == last.intent
                    && action["admission_face"]["identity"].is_string()
                    && action["admission_face"]["revision"]
                        .as_u64()
                        .is_some_and(|revision| revision > face.revision)
                    && action["accepted_effects"] == 0
                    && action["before"] == action["after"]
                    && action["admission_face"]["identity"] != shown["face"]["identity"]
            }))
        },
    );
    let released = hid_qmp::send_named_keys(
        stream,
        reader,
        &["ret"],
        false,
        "journey-release-stale-enter",
    );
    observed.and(released)?;
    artifacts.capture(stream, reader, "stale-refused", true)?;
    // One Escape leaves this ordinary inspector; the caller's existing Escape
    // opens Home. The later normal Stop establishes fresh-action recovery.
    journey_input::key_pair(stream, reader, "esc", "journey-stale-return-application")?;
    hid_qmp::wait_for_stage(
        serial_path,
        child,
        "CONDUIT_WORKSPACE_FACE returned-to-application",
        "journey-stale-return-timeout",
    )?;
    Ok(())
}
