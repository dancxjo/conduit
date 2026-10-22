//! Actual canonical Mask reading, correlated to its acknowledged owner Face.
use super::super::{qemu_artifacts::Artifacts, qmp};
use super::{
    face_correspondence, hid_qmp, io_error, journey_input, refusal, stage_count, ConduitosError,
};
use std::{fs, os::unix::net::UnixStream, path::Path, process::Child};

pub(super) fn exercise(
    serial: &Path,
    child: &mut Child,
    stream: &mut UnixStream,
    reader: &mut qmp::Reader,
    artifacts: &mut Artifacts,
) -> Result<(), ConduitosError> {
    let original =
        face_correspondence::current_patchbay_show(&fs::read_to_string(serial).map_err(io_error)?)?;
    for (key, usage, frame, diagram, details) in [
        ("f3", 60, "patchbay-diagram", true, false),
        ("1", 30, "patchbay-gear-details", false, true),
        ("f3", 60, "patchbay-diagram-returned", true, false),
    ] {
        let before = stage_count(serial, "CONDUIT_NATIVE_READING ")?;
        journey_input::held_key(
            |down| hid_qmp::send_named_keys(stream, reader, &[key], down, "journey-graph-reading"),
            || {
                hid_qmp::wait_for_stage_count(
                    serial,
                    child,
                    "CONDUIT_NATIVE_READING ",
                    before + 1,
                    "journey-graph-reading-timeout",
                )
            },
        )?;
        let text = fs::read_to_string(serial).map_err(io_error)?;
        let records = face_correspondence::records(&text, "CONDUIT_NATIVE_READING ")?;
        let current = records
            .last()
            .ok_or_else(|| refusal("missing graph reading receipt"))?;
        if current["usage"] != usage
            || current["diagram"] != diagram
            || (!diagram && current["details"] != details)
            || current["disposition"] != "mask-local-input-acknowledged-and-drawn"
        {
            return Err(refusal(
                "graph reading did not acknowledge its requested mode",
            ));
        }
        for field in [
            "face_id",
            "face_revision",
            "show_id",
            "manifestation_id",
            "host_id",
            "boot_id",
        ] {
            if current[field].is_null() || current[field] != original["show"][field] {
                return Err(refusal(format!("graph reading changed {field}")));
            }
        }
        if details {
            let focus = current["graph_focus"]
                .as_str()
                .ok_or_else(|| refusal("Details lacks exact graph focus"))?;
            if !original["face"]["subjects"]
                .as_array()
                .is_some_and(|subjects| {
                    subjects
                        .iter()
                        .any(|s| s["identity"] == focus && s["role"] == "Gear")
                })
            {
                return Err(refusal(
                    "Details focus is not an exact Gear from the owner Face",
                ));
            }
        }
        artifacts.capture(stream, reader, frame, true)?;
    }
    Ok(())
}
