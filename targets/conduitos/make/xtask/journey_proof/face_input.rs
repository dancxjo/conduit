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
