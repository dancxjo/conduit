//! Real offered Patchbay actions, correlated with the guest's current Face/Show.
use super::super::{qemu_artifacts::Artifacts, qmp};
use super::{face_correspondence, hid_qmp, io_error, journey_input, refusal, ConduitosError};
use conduit_face::Face;
use serde_json::{json, Value};
use std::{fs, os::unix::net::UnixStream, path::Path, process::Child};

const ACTION: &str = "CONDUIT_NATIVE_ACTION ";
const EDITOR: &str = "CONDUIT_NATIVE_SOURCE_EDITOR ";

pub(super) fn exercise(
    serial: &Path,
    child: &mut Child,
    qmp: &mut UnixStream,
    reader: &mut qmp::Reader,
    artifacts: &mut Artifacts,
) -> Result<Value, ConduitosError> {
    // The previous graph proof leaves the diagram visible. Leave it through
    // the actual reading entrance before traversing offered semantic controls.
    let reading_count = records(serial, "CONDUIT_NATIVE_READING ")?.len();
    journey_input::held_key(
        |down| hid_qmp::send_named_keys(qmp, reader, &["f3"], down, "patchbay-leave-diagram"),
        || {
            journey_input::wait_for_record(
                serial,
                child,
                "patchbay-reading-timeout",
                "patchbay-reading",
                |text| {
                    Ok(
                        face_correspondence::records(text, "CONDUIT_NATIVE_READING ")?.len()
                            > reading_count,
                    )
                },
            )
        },
    )?;
    let initial = current(serial)?;
    let face = decode(&initial)?;
    // The current native owner offers only its already selected graph. Enact
    // that exact offered action; cross-Plot selection remains a product gap.
    let select = face
        .actions
        .iter()
        .find(|action| {
            action.intent == "patchbay/select-plot@1"
                && face.properties.iter().any(|property| {
                    property.subject == action.target
                        && property.name == "patchbay-selected"
                        && property.value == conduit_face::FacePropertyValue::Flag(true)
                })
        })
        .ok_or_else(|| refusal("Patchbay does not offer its current selected Plot"))?;
    let target = select.target.clone();
    let selected = submit(serial, child, qmp, reader, &initial, &select.identity, None)?;
    let after_select = current(serial)?;
    if !decode(&after_select)?.properties.iter().any(|property| {
        property.subject == target
            && property.name == "patchbay-selected"
            && property.value == conduit_face::FacePropertyValue::Flag(true)
    }) {
        return Err(refusal("Patchbay did not select the exact offered Plot"));
    }
    if selected["before"]["active_play_id"] != selected["after"]["active_play_id"] {
        return Err(refusal("Patchbay inspection replaced the actor Play"));
    }
    artifacts.capture(qmp, reader, "patchbay-inspect-current-plot", false)?;

    // The existing editor proof intentionally left café. Edit must preserve
    // that real draft, not overwrite it to make this demonstration succeed.
    let editor_count = records(serial, EDITOR)?.len();
    let edit_face = decode(&after_select)?;
    let edit = edit_face
        .actions
        .iter()
        .find(|action| action.intent == "patchbay/edit-subject@1")
        .ok_or_else(|| refusal("Patchbay lacks its checked Edit action"))?;
    let edit_request = submit(
        serial,
        child,
        qmp,
        reader,
        &after_select,
        &edit.identity,
        Some(0),
    )?;
    let after_edit = current(serial)?;
    if records(serial, EDITOR)?.len() != editor_count
        || !serde_json::to_string(&after_edit["face"])
            .map_err(|error| refusal(error.to_string()))?
            .contains("native-editor-existing-draft")
        || edit_request["before"]["active_play_id"] != edit_request["after"]["active_play_id"]
    {
        return Err(refusal(
            "Patchbay Edit did not explicitly preserve the existing draft",
        ));
    }
    artifacts.capture(qmp, reader, "patchbay-existing-draft-refused", true)?;

    let mask_face = decode(&after_edit)?;
    let mask = mask_face
        .actions
        .iter()
        .find(|action| action.intent == "patchbay/change-masks@1")
        .ok_or_else(|| refusal("Patchbay lacks its admitted Mask action"))?;
    let mask_request = submit(
        serial,
        child,
        qmp,
        reader,
        &after_edit,
        &mask.identity,
        None,
    )?;
    let after_mask = current(serial)?;
    if mask_request["before"]["active_play_id"].is_null()
        || mask_request["after"]["active_play_id"].is_null()
        || mask_request["before"]["active_play_id"] == mask_request["after"]["active_play_id"]
        || mask_request["before"]["body_id"] != mask_request["after"]["body_id"]
    {
        return Err(refusal(
            "Patchbay Mask request did not replan the same Body",
        ));
    }
    artifacts.capture(qmp, reader, "patchbay-mask-replanned", true)?;
    Ok(
        json!({"select_plot":selected, "selection_kind":"idempotent-current-plot",
        "cross_plot_selection_enacted":false, "selected_face":after_select,
        "edit_request":edit_request, "edit_outcome":"native-editor-existing-draft",
        "successful_edit_handoff_enacted":false, "edit_refusal_face":after_edit,
        "mask_request":mask_request, "replanned_face":after_mask,
        "limits":"idempotent selection only; existing draft refusal; no cross-Plot selection, successful Source editor handoff or Save/reopen"}),
    )
}

fn decode(record: &Value) -> Result<Face, ConduitosError> {
    serde_json::from_value(record["face"].clone()).map_err(|error| refusal(error.to_string()))
}
fn records(serial: &Path, prefix: &str) -> Result<Vec<Value>, ConduitosError> {
    face_correspondence::records(&fs::read_to_string(serial).map_err(io_error)?, prefix)
}
fn current(serial: &Path) -> Result<Value, ConduitosError> {
    face_correspondence::current_patchbay_show(&fs::read_to_string(serial).map_err(io_error)?)
}

#[allow(clippy::too_many_arguments)]
fn submit(
    serial: &Path,
    child: &mut Child,
    qmp: &mut UnixStream,
    reader: &mut qmp::Reader,
    prior: &Value,
    action_id: &str,
    argument: Option<usize>,
) -> Result<Value, ConduitosError> {
    let face = decode(prior)?;
    let action = face
        .actions
        .iter()
        .position(|action| action.identity == action_id)
        .ok_or_else(|| refusal("Patchbay offered action disappeared"))?;
    let argument_name = argument
        .map(|index| {
            face.actions[action]
                .arguments
                .get(index)
                .map(|argument| argument.name.as_str())
                .ok_or_else(|| refusal("Patchbay argument is absent from the offered contract"))
        })
        .transpose()?;
    let limit = face
        .actions
        .iter()
        .map(|action| 1 + action.arguments.len())
        .sum::<usize>()
        + 1;
    let mut focused = false;
    for _ in 0..limit {
        let count = records(serial, "CONDUIT_NATIVE_READING ")?.len();
        journey_input::held_key(
            |down| {
                hid_qmp::send_named_keys(
                    qmp,
                    reader,
                    &["tab"],
                    down,
                    "patchbay-focus-offered-action",
                )
            },
            || {
                journey_input::wait_for_record(
                    serial,
                    child,
                    "patchbay-focus-timeout",
                    "patchbay-focus",
                    |text| {
                        Ok(
                            face_correspondence::records(text, "CONDUIT_NATIVE_READING ")?.len()
                                > count,
                        )
                    },
                )
            },
        )?;
        let readings = records(serial, "CONDUIT_NATIVE_READING ")?;
        let reading = readings
            .last()
            .ok_or_else(|| refusal("Patchbay Tab lacks a reading receipt"))?;
        if reading["usage"] != 43
            || reading["face_id"] != prior["face"]["identity"]
            || reading["face_revision"] != prior["face"]["revision"]
            || reading["show_id"] != prior["show"]["show_id"]
            || reading["disposition"] != "mask-local-input-acknowledged-and-drawn"
        {
            return Err(refusal(
                "Patchbay focus is not acknowledged against the offered Show",
            ));
        }
        if reading["action_id"] == action_id && reading["argument_name"] == json!(argument_name) {
            focused = true;
            break;
        }
    }
    if !focused {
        return Err(refusal(
            "offered Patchbay control is absent from bounded actual Tab traversal",
        ));
    }
    if argument.is_some() {
        let count = records(serial, "CONDUIT_NATIVE_READING ")?.len();
        journey_input::held_key(
            |down| {
                hid_qmp::send_named_keys(
                    qmp,
                    reader,
                    &["right"],
                    down,
                    "patchbay-select-offered-subject",
                )
            },
            || {
                journey_input::wait_for_record(
                    serial,
                    child,
                    "patchbay-choice-timeout",
                    "patchbay-choice",
                    |text| {
                        Ok(
                            face_correspondence::records(text, "CONDUIT_NATIVE_READING ")?.len()
                                > count,
                        )
                    },
                )
            },
        )?;
    }
    let count = records(serial, ACTION)?.len();
    journey_input::held_key(
        |down| {
            hid_qmp::send_named_keys(
                qmp,
                reader,
                &["ret"],
                down,
                "patchbay-submit-offered-action",
            )
        },
        || {
            journey_input::wait_for_record(
                serial,
                child,
                "patchbay-action-timeout",
                "patchbay-action",
                |text| Ok(face_correspondence::records(text, ACTION)?.len() > count),
            )
        },
    )?;
    let actions = records(serial, ACTION)?;
    if actions.len() != count + 1 {
        return Err(refusal("Patchbay produced extra owner actions"));
    }
    let receipt = actions.last().unwrap();
    if receipt["interaction"]["action_id"] != action_id
        || receipt["show_id"] != prior["show"]["show_id"]
        || receipt["face_id"] != prior["face"]["identity"]
        || receipt["face_revision"] != prior["face"]["revision"]
        || receipt["disposition"].get("Accepted").is_none()
    {
        return Err(refusal(
            "Patchbay owner action differs from offered predecessor",
        ));
    }
    // Action acceptance precedes replacement presentation. Wait for the actual
    // fresh publication before another input or screenshot.
    journey_input::wait_for_record(
        serial,
        child,
        "patchbay-new-show-timeout",
        "patchbay-new-show",
        |text| {
            Ok(face_correspondence::records(text, "CONDUIT_NATIVE_FACE ")?
                .last()
                .is_some_and(|record| record["show"]["show_id"] != prior["show"]["show_id"]))
        },
    )?;
    Ok(receipt.clone())
}
