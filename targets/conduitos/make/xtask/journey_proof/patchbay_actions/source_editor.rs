//! Positive Source handoff after an offered Lull and fresh Wake/Plan/Play encounter.
use super::live_serial;
use super::*;
use conduitos::native_workset::{self, NativePlot};

fn journey(serial: &Path) -> Result<Value, ConduitosError> {
    super::super::journey_records::decode(&live_serial::read_complete(serial).map_err(io_error)?)?
        .pop()
        .ok_or_else(|| refusal("fresh editor journey is absent"))
}
fn home(
    serial: &Path,
    child: &mut Child,
    qmp: &mut UnixStream,
    reader: &mut qmp::Reader,
) -> Result<(), ConduitosError> {
    for _ in 0..2 {
        let before = live_serial::read_complete(serial).map_err(io_error)?;
        let homes = before.matches("CONDUIT_HOME_CHECKPOINT returned\n").count();
        let exits = before
            .matches("CONDUIT_WORKSPACE_FACE returned-to-application\n")
            .count();
        journey_input::held_key(
            |down| hid_qmp::send_named_keys(qmp, reader, &["esc"], down, "source-editor-open-home"),
            || {
                journey_input::wait_for_record(
                    serial,
                    child,
                    "source-editor-home-timeout",
                    "source-editor-home",
                    |text| {
                        Ok(
                            text.matches("CONDUIT_HOME_CHECKPOINT returned\n").count() > homes
                                || text
                                    .matches("CONDUIT_WORKSPACE_FACE returned-to-application\n")
                                    .count()
                                    > exits,
                        )
                    },
                )
            },
        )?;
        if live_serial::read_complete(serial)
            .map_err(io_error)?
            .matches("CONDUIT_HOME_CHECKPOINT returned\n")
            .count()
            > homes
        {
            return Ok(());
        }
    }
    Err(refusal(
        "fresh editor cannot reach the ordinary Home picker",
    ))
}
fn edited(
    serial: &Path,
    child: &mut Child,
    qmp: &mut UnixStream,
    reader: &mut qmp::Reader,
    key: &str,
    text: &str,
    before: &Value,
) -> Result<Value, ConduitosError> {
    journey_input::held_key(
        |down| {
            hid_qmp::send_named_keys(
                qmp,
                reader,
                &[key],
                down,
                "source-editor-real-revisioned-key",
            )
        },
        || {
            journey_input::wait_for_record(
                serial,
                child,
                "source-editor-text-timeout",
                "source-editor-text",
                |encoded| {
                    Ok(super::super::journey_records::decode(encoded)?
                        .last()
                        .is_some_and(|record| {
                            record["result"] == text
                                && record["input_sign_id"] != before["input_sign_id"]
                                && record["active_play_id"] == before["active_play_id"]
                                && record["checked_plot_id"] == before["checked_plot_id"]
                        }))
                },
            )
        },
    )?;
    journey(serial)
}
pub(in super::super) fn exercise(
    serial: &Path,
    child: &mut Child,
    qmp: &mut UnixStream,
    reader: &mut qmp::Reader,
    artifacts: &mut Artifacts,
) -> Result<Value, ConduitosError> {
    let stopped = journey(serial)?;
    let previous_live = super::super::journey_records::decode(
        &live_serial::read_complete(serial).map_err(io_error)?,
    )?
    .into_iter()
    .rev()
    .find(|record| !record["active_play_id"].is_null())
    .ok_or_else(|| refusal("fresh editor lacks the prior retired Play identity"))?;
    if stopped["status"] != "stopped" {
        return Err(refusal(
            "positive editor must follow retained stopped reentry",
        ));
    }
    let mut lifecycle = Vec::new();
    for (key, status, intent) in [
        ("f7", "lulled", "conduit.intent/lull@1"),
        ("f4", "awake", "conduit.intent/wake@1"),
        ("f5", "planned", "conduit.intent/plan@1"),
        ("f6", "quiescent-awaiting-input", "conduit.intent/play@1"),
    ] {
        let count = records(serial, ACTION)?.len();
        let before = journey(serial)?;
        journey_input::held_key(
            |down| {
                hid_qmp::send_named_keys(qmp, reader, &[key], down, "source-editor-fresh-lifecycle")
            },
            || {
                journey_input::wait_for_record(
                    serial,
                    child,
                    "source-editor-lifecycle-timeout",
                    status,
                    |text| {
                        Ok(face_correspondence::records(text, ACTION)?.len() > count
                            && super::super::journey_records::decode(text)?
                                .last()
                                .is_some_and(|record| {
                                    record["status"] == status
                                        && record["revision"]
                                            .as_u64()
                                            .zip(before["revision"].as_u64())
                                            .is_some_and(|(now, prior)| now > prior)
                                }))
                    },
                )
            },
        )?;
        let actions = records(serial, ACTION)?;
        let action = actions
            .get(count)
            .ok_or_else(|| refusal("fresh lifecycle lacks an actual typed action receipt"))?;
        if actions.len() != count + 1
            || action["requested_intent"] != intent
            || action["disposition"].get("Accepted").is_none()
        {
            return Err(refusal(
                "fresh editor lifecycle differs from the offered typed action",
            ));
        }
        lifecycle.push(action.clone());
    }
    let fresh = journey(serial)?;
    if fresh["body_id"] != stopped["body_id"]
        || fresh["plan_id"].is_null()
        || fresh["active_play_id"].is_null()
        || fresh["plan_id"] == previous_live["plan_id"]
        || fresh["active_play_id"] == previous_live["active_play_id"]
    {
        return Err(refusal(
            "fresh editor did not receive a new exact Plan and Play on the same Body",
        ));
    }
    home(serial, child, qmp, reader)?;
    journey_input::open_hub_plot(qmp, reader, serial, child, NativePlot::MemoryLantern)?;
    let empty = journey(serial)?;
    if empty["result"] != ""
        || empty["text_editor_active"] != true
        || empty["active_play_id"] != fresh["active_play_id"]
    {
        return Err(refusal(
            "fresh admitted document is not independently observed empty",
        ));
    }
    home(serial, child, qmp, reader)?;
    journey_input::open_hub_plot(qmp, reader, serial, child, NativePlot::Patchbay)?;
    let initial = current(serial)?;
    let resident = native_workset::resident(NativePlot::MemoryLantern)
        .map_err(|error| refusal(error.as_str()))?;
    let target = format!("plot/{}", resident.checked_plot_id.as_str());
    let face = decode(&initial)?;
    let select = face
        .actions
        .iter()
        .find(|action| action.intent == "patchbay/select-plot@1" && action.target == target)
        .ok_or_else(|| refusal("fresh Patchbay does not offer the exact document Source"))?;
    let selected = submit(serial, child, qmp, reader, &initial, &select.identity, None)?;
    let prior = current(serial)?;
    let correspondence = selection::source(&prior, &target)?;
    let face = decode(&prior)?;
    let edit = face
        .actions
        .iter()
        .find(|action| action.intent == "patchbay/edit-subject@1")
        .ok_or_else(|| refusal("fresh Source lacks its offered Edit action"))?;
    let count = records(serial, EDITOR)?.len();
    let request = submit(serial, child, qmp, reader, &prior, &edit.identity, Some(0))?;
    let receipts = records(serial, EDITOR)?;
    if receipts.len() != count + 1 {
        return Err(refusal(
            "fresh Source handoff lacks exactly one actual receipt",
        ));
    }
    let receipt = &receipts[count];
    let checked = native_workset::checked(NativePlot::MemoryLantern)
        .map_err(|error| refusal(error.as_str()))?;
    let source = NativePlot::MemoryLantern.source();
    let loaded = journey(serial)?;
    if receipt["source"] != json!(resident)
        || receipt["expanded_plot_id"] != json!(checked.expanded_plot_id)
        || receipt["body_plan_id"] != fresh["plan_id"]
        || receipt["active_play_id"] != fresh["active_play_id"]
        || receipt["selection_enacted"] != false
        || loaded["result"] != source
        || loaded["result_omitted_bytes"] != 0
        || loaded["text_editor_active"] != true
        || loaded["checked_plot_id"] != resident.checked_plot_id.as_str()
        || loaded["active_play_id"] != fresh["active_play_id"]
    {
        return Err(refusal(
            "actual Source editor state differs from the exact authorized Source handoff",
        ));
    }
    let input_subject = request["interaction"]["arguments"]
        .as_array()
        .and_then(|args| args.iter().find(|value| value["name"] == "subject"))
        .ok_or_else(|| refusal("Source handoff has no exact typed subject"))?;
    let bytes: Vec<u8> = serde_json::from_value(input_subject["value"].clone())
        .map_err(|error| refusal(error.to_string()))?;
    if receipt["subject"] != String::from_utf8(bytes).map_err(|error| refusal(error.to_string()))? {
        return Err(refusal(
            "Source receipt changed the acknowledged Edit subject",
        ));
    }
    let suffix = edited(
        serial,
        child,
        qmp,
        reader,
        "x",
        &format!("{source}x"),
        &loaded,
    )?;
    let restored = edited(serial, child, qmp, reader, "backspace", source, &suffix)?;
    let face_record = records(serial, "CONDUIT_NATIVE_FACE ")?
        .pop()
        .ok_or_else(|| refusal("fresh document Face absent"))?;
    let document = decode(&face_record)?;
    if !document.subjects.iter().any(|subject| {
        subject.role == conduit_face::FaceRole::Document
            && document
                .text
                .iter()
                .any(|text| text.subject == subject.identity && text.text == source)
    }) {
        return Err(refusal(
            "fresh editor does not expose the complete authoritative document text",
        ));
    }
    artifacts.capture(qmp, reader, "patchbay-source-editor-loaded", true)?;
    let result = json!({"stopped":stopped,"previous_live":previous_live,"lifecycle":lifecycle,"fresh":fresh,"empty_editor":empty,
        "selection":selected,"source_correspondence":correspondence,"typed_request":request,
        "source_edit_receipt":receipt,"loaded":loaded,"suffix":suffix,"restored":restored,"document_face":face_record,
        "numeric_text_document_revision":"not exposed by current receipt; actual revisioned suffix/backspace enacted",
        "selection_enacted":false,"save_reopen_enacted":false});
    let before_stop = journey(serial)?;
    let actions_before_stop = records(serial, ACTION)?.len();
    journey_input::held_key(
        |down| {
            hid_qmp::send_named_keys(qmp, reader, &["f8"], down, "source-editor-stop-fresh-play")
        },
        || {
            journey_input::wait_for_record(
                serial,
                child,
                "source-editor-stop-timeout",
                "source-editor-stopped",
                |text| {
                    Ok(
                        face_correspondence::records(text, ACTION)?.len() > actions_before_stop
                            && super::super::journey_records::decode(text)?
                                .last()
                                .is_some_and(|record| {
                                    record["status"] == "stopped"
                                        && record["revision"]
                                            .as_u64()
                                            .zip(before_stop["revision"].as_u64())
                                            .is_some_and(|(now, prior)| now > prior)
                                }),
                    )
                },
            )
        },
    )?;
    Ok(result)
}
