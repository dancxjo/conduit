//! Current universal Face/Show ownership through real Mask-local inspection.
use super::super::{qemu_artifacts::Artifacts, qmp};
use super::{face_correspondence, io_error, timing_interactions, ConduitosError};
use serde_json::Value;
use std::{fs, os::unix::net::UnixStream, path::Path, process::Child};

fn refusal(detail: &str) -> ConduitosError {
    ConduitosError::refusal("journey-mask-inspection", detail)
}

pub(super) fn current(serial: &Path, owner: &Value) -> Result<Value, ConduitosError> {
    let text = fs::read_to_string(serial).map_err(io_error)?;
    let record = face_correspondence::records(&text, "CONDUIT_NATIVE_FACE ")?
        .pop()
        .ok_or_else(|| refusal("selected Plot has no acknowledged universal Face"))?;
    let face: conduit_face::Face = serde_json::from_value(record["face"].clone())
        .map_err(|_| refusal("invalid current Face"))?;
    face.validate()
        .map_err(|_| refusal("current Face identity is invalid"))?;
    for field in [
        "body_id",
        "plan_id",
        "active_play_id",
        "checked_plot_id",
        "source_document_id",
    ] {
        if owner[field].as_str().is_none() || record["face"]["basis"][field] != owner[field] {
            return Err(refusal(
                "current Face differs from its selected Body/Plan/Play/Source",
            ));
        }
    }
    for field in ["host_id", "boot_id"] {
        if owner[field].as_str().is_none() || record["show"][field] != owner[field] {
            return Err(refusal("current Show belongs to a different Host/Boot"));
        }
    }
    let show = &record["show"];
    if show["face_id"] != record["face"]["identity"]
        || show["face_revision"] != record["face"]["revision"]
        || show["lifecycle"] != "Available"
        || show["mask_plan_id"].as_str().is_none_or(str::is_empty)
        || show["mask_active_play_id"]
            .as_str()
            .is_none_or(str::is_empty)
    {
        return Err(refusal("selected Face lacks its exact available Mask Show"));
    }
    Ok(record)
}

fn validate_reading(
    record: &Value,
    face: &Value,
    usage: u64,
    details: bool,
    diagram: bool,
) -> Result<(), ConduitosError> {
    if record["schema"] != "conduit.native-reading/v1"
        || record["usage"] != usage
        || record["disposition"] != "mask-local-input-acknowledged-and-drawn"
        || record["details"] != details
        || record["diagram"] != diagram
    {
        return Err(refusal(
            "inspection did not reach its exact acknowledged Mask mode",
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
        if face["show"][field].is_null() || record[field] != face["show"][field] {
            return Err(refusal(
                "local inspection changed its immutable Face/Show owner",
            ));
        }
    }
    Ok(())
}

pub(super) fn exercise(
    serial: &Path,
    child: &mut Child,
    stream: &mut UnixStream,
    reader: &mut qmp::Reader,
    artifacts: &mut Artifacts,
    face: &Value,
) -> Result<(), ConduitosError> {
    for (key, usage, details, diagram, frame) in [
        ("f3", 60, false, true, "patchbay-diagram"),
        ("1", 30, true, false, "patchbay-gear-details"),
        // Returning to the diagram retains the inspected Details state.
        ("f3", 60, true, true, "patchbay-diagram-returned"),
    ] {
        let record = timing_interactions::press(serial, child, stream, reader, key, usage)?;
        validate_reading(&record, face, usage, details, diagram)?;
        artifacts.capture(stream, reader, frame, true)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn inspection_requires_the_exact_acknowledged_mode_and_owner() {
        let show = json!({"face_id":"face", "face_revision":3,"show_id":"show", "manifestation_id":"manifestation", "host_id":"host", "boot_id":"boot"});
        let face = json!({"show":show});
        let mut record = face["show"].clone();
        record["schema"] = json!("conduit.native-reading/v1");
        record["usage"] = json!(60);
        record["disposition"] = json!("mask-local-input-acknowledged-and-drawn");
        record["details"] = json!(false);
        record["diagram"] = json!(true);
        validate_reading(&record, &face, 60, false, true).unwrap();
        for field in [
            "schema",
            "usage",
            "disposition",
            "details",
            "diagram",
            "face_id",
            "face_revision",
            "show_id",
            "manifestation_id",
            "host_id",
            "boot_id",
        ] {
            let mut wrong = record.clone();
            wrong[field] = Value::Null;
            assert!(
                validate_reading(&wrong, &face, 60, false, true).is_err(),
                "{field}"
            );
        }
    }
}
