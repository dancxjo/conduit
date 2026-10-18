//! Real keyboard replacement of a source-owned Boolean, then its restoration.
use super::super::{qemu_artifacts::Artifacts, qmp};
use super::{face_correspondence, hid_qmp, io_error, journey_input, refusal, ConduitosError};
use conduit_face::{Face, FaceInteraction, FacePropertyValue};
use serde_json::{json, Value};
use std::{fs, os::unix::net::UnixStream, path::Path, process::Child};

pub(super) fn exercise(
    serial: &Path,
    child: &mut Child,
    stream: &mut UnixStream,
    reader: &mut qmp::Reader,
    artifacts: &mut Artifacts,
) -> Result<(), ConduitosError> {
    navigate_to(serial, child, stream, reader, "creche.plot.0", None, false)?;
    for (key, expected, frame) in [
        ("spc", false, "creche-three-selected"),
        ("ret", true, "creche-four-restored"),
    ] {
        let before = face_correspondence::records(
            &fs::read_to_string(serial).map_err(io_error)?,
            "CONDUIT_CRECHE_CHANGE ",
        )?
        .len();
        journey_input::key_pair(stream, reader, key, "creche-reversible-choice")?;
        journey_input::wait_for_record_retaining_guest(
            serial,
            child,
            "creche-choice-timeout",
            frame,
            |text| Ok(face_correspondence::records(text, "CONDUIT_CRECHE_CHANGE ")?.len() > before),
        )?;
        validate(&fs::read_to_string(serial).map_err(io_error)?, expected)?;
        artifacts.capture(stream, reader, frame, true)?;
    }
    navigate_to(
        serial,
        child,
        stream,
        reader,
        "creche.name",
        Some("value"),
        true,
    )?;
    Ok(())
}

fn navigate_to(
    serial: &Path,
    child: &mut Child,
    stream: &mut UnixStream,
    reader: &mut qmp::Reader,
    action: &str,
    argument: Option<&str>,
    reverse: bool,
) -> Result<(), ConduitosError> {
    let text = fs::read_to_string(serial).map_err(io_error)?;
    let faces = face_correspondence::records(&text, "CONDUIT_NATIVE_FACE ")?;
    let current = faces
        .last()
        .ok_or_else(|| refusal("missing current Crèche Face"))?;
    let face: Face =
        serde_json::from_value(current["face"].clone()).map_err(|e| refusal(e.to_string()))?;
    face.validate().map_err(|e| refusal(e.to_string()))?;
    if !face.actions.iter().any(|candidate| {
        candidate.identity == action
            && candidate.availability.is_available()
            && argument.is_none_or(|name| candidate.arguments.iter().any(|arg| arg.name == name))
    }) {
        return Err(refusal(
            "Crèche target is not offered by the current source Face",
        ));
    }
    let bound = face
        .actions
        .iter()
        .map(|action| action.arguments.len() + 1)
        .sum::<usize>();
    for step in 0..=bound {
        let text = fs::read_to_string(serial).map_err(io_error)?;
        let focus = face_correspondence::records(&text, "CONDUIT_CRECHE_FOCUS ")?;
        let selected = focus
            .last()
            .ok_or_else(|| refusal("missing observed Crèche focus"))?;
        if selected["schema"] != "conduit.creche-focus/v1"
            || selected["face_id"] != json!(face.identity)
            || selected["face_revision"] != face.revision
            || selected["show_id"] != current["show"]["show_id"]
        {
            return Err(refusal(
                "Crèche focus does not bind the current acknowledged Face/Show",
            ));
        }
        if selected["action_id"] == action && selected["argument_name"] == json!(argument) {
            return Ok(());
        }
        if step == bound {
            break;
        }
        let before = focus.len();
        let keys: &[&str] = if reverse { &["shift", "tab"] } else { &["tab"] };
        hid_qmp::send_named_keys(stream, reader, keys, true, "creche-choice-focus")?;
        hid_qmp::send_named_keys(stream, reader, keys, false, "creche-choice-focus")?;
        journey_input::wait_for_record_retaining_guest(
            serial,
            child,
            "creche-choice-focus-timeout",
            "focus",
            |text| Ok(face_correspondence::records(text, "CONDUIT_CRECHE_FOCUS ")?.len() > before),
        )?;
    }
    Err(refusal(
        "Crèche target not reached within the admitted control bound",
    ))
}

fn validate(serial: &str, expected: bool) -> Result<Value, ConduitosError> {
    let changes = face_correspondence::records(serial, "CONDUIT_CRECHE_CHANGE ")?;
    let change = changes
        .last()
        .ok_or_else(|| refusal("missing accepted Crèche edit"))?;
    let faces = face_correspondence::records(serial, "CONDUIT_NATIVE_FACE ")?;
    let find = |side: &str| -> Result<&Value, ConduitosError> {
        faces
            .iter()
            .find(|record| {
                record["face"]["identity"] == change[format!("{side}_face_id")]
                    && record["show"]["show_id"] == change[format!("{side}_show_id")]
            })
            .ok_or_else(|| refusal("Crèche edit lacks exact acknowledged source Face/Show"))
    };
    let before = find("before")?;
    let after = find("after")?;
    let old: Face =
        serde_json::from_value(before["face"].clone()).map_err(|e| refusal(e.to_string()))?;
    let fresh: Face =
        serde_json::from_value(after["face"].clone()).map_err(|e| refusal(e.to_string()))?;
    old.validate().map_err(|e| refusal(e.to_string()))?;
    fresh.validate().map_err(|e| refusal(e.to_string()))?;
    let interaction: FaceInteraction = serde_json::from_value(change["interaction"].clone())
        .map_err(|e| refusal(e.to_string()))?;
    let action = old
        .actions
        .iter()
        .find(|a| {
            a.identity == "creche.plot.0"
                && a.identity == interaction.action_id
                && a.target == interaction.target
                && a.availability.is_available()
        })
        .ok_or_else(|| refusal("Crèche replacement lacks exact current available Plot action"))?;

    if change["schema"] != "conduit.creche-change/v1"
        || change["disposition"] != "source-accepted-and-shown"
        || interaction.face_id != old.identity.as_str()
        || interaction.face_revision != old.revision
        || json!(interaction.show_id) != before["show"]["show_id"]
        || interaction.arguments.len() != 1
        || action.arguments.len() != 1
        || interaction.arguments[0].name != action.arguments[0].name
        || interaction.arguments[0].value_kind != action.arguments[0].contract.value_kind.as_str()
        || change["before_revision"] != old.revision
        || change["after_revision"] != fresh.revision
        || interaction.arguments[0].value != [u8::from(expected)]
        || action.arguments[0]
            .contract
            .validate(&interaction.arguments[0].value)
            .is_err()
        || selected(&old, &action.target) != Some(!expected)
        || selected(&fresh, &action.target) != Some(expected)
        || old.identity == fresh.identity
        || old.revision >= fresh.revision
        || old.basis != fresh.basis
        || old.basis.body_id.is_some()
        || before["show"]["show_id"] == after["show"]["show_id"]
        || ["host_id", "boot_id", "offer_generation"]
            .iter()
            .any(|field| before["show"][*field] != after["show"][*field])
        || fresh
            .properties
            .iter()
            .filter(|p| p.name == "selected" && p.value == FacePropertyValue::Flag(true))
            .count()
            != if expected { 4 } else { 3 }
        || !fresh
            .actions
            .iter()
            .any(|a| a.identity == "creche.birth" && a.availability.is_available())
    {
        return Err(refusal(
            "Crèche Boolean replacement changed provenance or lacks fresh checked value and Show",
        ));
    }
    Ok(change.clone())
}

fn selected(face: &Face, target: &str) -> Option<bool> {
    face.properties.iter().find_map(|p| {
        if p.subject != target || p.name != "selected" {
            return None;
        }
        match &p.value {
            FacePropertyValue::Flag(value) => Some(*value),
            _ => None,
        }
    })
}
