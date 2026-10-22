//! Real Page Up/Down acknowledgements in the current Mask's Details reader.
use super::super::{qemu_artifacts::Artifacts, qmp};
use super::{face_correspondence, hid_qmp, io_error, journey_input, ConduitosError};
use serde_json::{json, Value};
use std::{fs, os::unix::net::UnixStream, path::Path, process::Child};

const PREFIX: &str = "CONDUIT_NATIVE_READING ";
fn refusal(detail: &str) -> ConduitosError {
    ConduitosError::refusal("native-reading-observation", detail)
}
fn records(serial: &Path) -> Result<Vec<Value>, ConduitosError> {
    let bytes = fs::read(serial).map_err(io_error)?;
    face_correspondence::records(journey_input::complete_records(&bytes)?, PREFIX)
}
fn press(
    serial: &Path,
    child: &mut Child,
    stream: &mut UnixStream,
    reader: &mut qmp::Reader,
    key: &str,
    usage: u64,
) -> Result<Value, ConduitosError> {
    let before = records(serial)?.len();
    journey_input::held_key(
        |down| hid_qmp::send_named_keys(stream, reader, &[key], down, "native-reading-observation"),
        || {
            journey_input::wait_for_record(serial, child, "native-reading-timeout", key, |text| {
                let found = face_correspondence::records(text, PREFIX)?;
                Ok(found.len() > before && found.last().is_some_and(|r| r["usage"] == usage))
            })
        },
    )?;
    records(serial)?
        .pop()
        .ok_or_else(|| refusal("missing acknowledged reading input"))
}

pub(super) fn reading_pages(
    serial: &Path,
    child: &mut Child,
    stream: &mut UnixStream,
    reader: &mut qmp::Reader,
    artifacts: &mut Artifacts,
) -> Result<(), ConduitosError> {
    let mut rows = Vec::with_capacity(3);
    let mut first = None;
    for (key, usage, name) in [
        ("f2", 59, "reading-details-first"),
        ("pgdn", 78, "reading-details-next"),
        ("pgup", 75, "reading-details-returned"),
    ] {
        let start = reader.observer_elapsed_ns();
        let record = press(serial, child, stream, reader, key, usage)?;
        let accepted = reader.observer_elapsed_ns();
        if record["pages"].as_u64().is_none_or(|pages| pages < 2)
            || record["native_observer"]["clock"] != "x86-tsc-lfence"
            || record["native_observer"]["ticks"].as_u64().is_none()
        {
            return Err(refusal("actual Details reader needs multiple timed pages"));
        }
        if let Some(initial) = &first {
            let initial: &Value = initial;
            for field in ["face_id", "face_revision", "show_id", "host_id", "boot_id"] {
                if record[field] != initial[field] {
                    return Err(refusal("local page navigation changed its Face/Show owner"));
                }
            }
            let expected = if usage == 78 { 1 } else { 0 };
            if record["page"] != expected {
                return Err(refusal(
                    "page navigation did not reach the exact expected page",
                ));
            }
        } else {
            if record["page"] != 0 {
                return Err(refusal("Details did not start on page zero"));
            }
            first = Some(record.clone());
        }
        artifacts.capture(stream, reader, name, true)?;
        rows.push(json!({"key":key,"command_start_ns":start,
            "accepted_record_observed_ns":accepted,"frame_observed_ns":reader.observer_elapsed_ns(),
            "guest_reading_record":record,"frame_checkpoint":name}));
        fs::write(serial.with_file_name("journey-reading-timing.json"), serde_json::to_vec_pretty(&json!({
            "schema":"conduit.conduitos/native-reading-observation@1","clock":"std::time::Instant",
            "units":"nanoseconds","origin":"shared QMP/frame observation origin",
            "interpretation":"Current native reader pagination, not continuous wheel scrolling. Host intervals include input dispatch, receipt polling and capture; guest records retain exact Face/Show ownership.",
            "rows":rows
        })).map_err(|_| refusal("cannot encode reading observations"))?).map_err(io_error)?;
    }
    // Restore the primary view before the unchanged diagram/lifecycle journey.
    press(serial, child, stream, reader, "f2", 59)?;
    Ok(())
}
