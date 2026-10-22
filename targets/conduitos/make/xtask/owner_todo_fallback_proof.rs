//! Reachable required-item refusal on an isolated real owner route.
//! Never resubmit a consumed action: stale admission is checked locally only.
use super::*;
use conduit_face::{Face, FaceInteraction, ManifestationLifecycle, MaskShow};
use std::{os::unix::net::UnixStream, path::Path, time::Instant};

const COMPONENT: &str = "CONDUIT_NATIVE_COMPONENT_REALIZATION ";

#[allow(clippy::too_many_arguments)]
pub(super) fn exercise(
    args: &LiveOwnerTodoActionProofArgs,
    directory: &Path,
    serial: &Path,
    child: &mut Child,
    qmp: &mut UnixStream,
    reader: &mut qmp::Reader,
    before: &Value,
) -> Result<Value, ConduitosError> {
    let owner_before = todo_validation::read_and_validate_current(
        &args.face,
        before,
        &args.face.expected_status,
        Some(&args.expected_action_id),
    )?;
    let baseline = latest(serial)?;
    let (original, _) = decode(&baseline)?;
    if serde_json::to_value(&original).ok().as_ref() != Some(&owner_before["face"])
        || baseline["expression"]["expression"]["density"] != "compact"
        || baseline["expression"]["expression"]["palette"] != "phosphor"
        || baseline["expression"]["expression"]["text_scale_percent"] != 100
    {
        return Err(refusal("native-todo-fallback-requires-compact-phosphor"));
    }
    select(qmp, reader, true, "f8")?;
    let selected = wait_component(serial, child, &baseline["acknowledged_show"]["show_id"])?;
    let (face, show) = decode(&selected)?;
    if face != original
        || !selected["fallback"].is_null()
        || selected["configuration"]["recipe"] == baseline["configuration"]["recipe"]
        || selected["expression"] != baseline["expression"]
    {
        return Err(refusal("native-todo-required-item-selection-invalid"));
    }
    let prepared = conduit_face::prepare_component_recipe(recipe(&selected)?, &face)
        .map_err(|_| refusal("native-todo-required-item-recipe-invalid"))?;
    if prepared.recipe.identity() != "conduit.face/todo-required-item@1" {
        return Err(refusal("native-todo-required-item-recipe-mismatch"));
    }
    let action = face
        .actions
        .iter()
        .find(|action| action.identity == args.expected_action_id)
        .ok_or_else(|| refusal("native-todo-required-complete-absent"))?;
    let stale = FaceInteraction::new(
        &face,
        &show,
        &args.expected_action_id,
        &action.target,
        vec![],
        1,
    )
    .map_err(|_| refusal("native-todo-required-complete-invalid"))?;
    let selected_image = capture(qmp, reader, directory, "required-item-selected")?;
    write_checkpoint(
        directory,
        "native-required-item-ready.json",
        &json!({
            "stage":"required-item-ready", "realization":selected, "owner_face":owner_before,
        }),
    )?;
    wait_for_resume(
        directory,
        "resume-native-action",
        child,
        Duration::from_secs(45),
    )?;
    let (first, fallback) = enact(
        args.tab_count,
        &args.expected_action_id,
        &selected,
        serial,
        child,
        qmp,
        reader,
    )?;
    validate_fallback(&selected, &fallback)?;
    let (post, post_show) = decode(&fallback)?;
    // A real removed action, rather than synthetic capacity pressure, must cause refusal.
    if post
        .actions
        .iter()
        .any(|action| action.identity == args.expected_action_id)
    {
        return Err(refusal("native-todo-required-action-still-available"));
    }
    let stale_refusal = stale
        .validate_against(&post, &post_show)
        .err()
        .ok_or_else(|| refusal("native-todo-former-show-was-not-stale"))?;
    let remaining = args
        .fallback_action_id
        .as_deref()
        .ok_or_else(|| refusal("native-todo-fallback-action-absent"))?;
    let post_guest = guest(&fallback)?;
    let owner_post = todo_validation::read_and_validate_current(
        &args.face,
        &post_guest,
        &args.expected_after_status,
        Some(remaining),
    )?;
    if owner_post["face"] != fallback["face"] {
        return Err(refusal("native-todo-fallback-owner-face-mismatch"));
    }
    let fallback_image = capture(qmp, reader, directory, "required-item-fallback")?;
    // Exercise the remaining action WHILE the generic fallback is still selected.
    let (second, after_remaining) = enact(
        args.fallback_tab_count.unwrap(),
        remaining,
        &fallback,
        serial,
        child,
        qmp,
        reader,
    )?;
    validate_fallback(&selected, &after_remaining)?;
    let final_guest = guest(&after_remaining)?;
    let owner_final = todo_validation::read_and_validate_current(
        &args.face,
        &final_guest,
        args.fallback_status.as_deref().unwrap(),
        None,
    )?;
    if owner_final["face"] != after_remaining["face"] {
        return Err(refusal("native-todo-fallback-final-owner-mismatch"));
    }
    let after_image = capture(qmp, reader, directory, "required-item-remaining-action")?;
    // The fourth/final refresh selects the ordinary flat component, providing an
    // independent presentation of this identical post-action Face and expression.
    select(qmp, reader, false, "f6")?;
    let comparison = wait_component(
        serial,
        child,
        &after_remaining["acknowledged_show"]["show_id"],
    )?;
    validate_comparison(&after_remaining, &comparison)?;
    let comparison_image = capture(qmp, reader, directory, "required-item-same-face-flat")?;
    let actions = action_records(serial)?;
    if actions.len() != 2 || actions[0] != first || actions[1] != second {
        return Err(refusal("native-todo-fallback-extra-owner-action"));
    }
    Ok(json!({
        "selected":selected, "completion":first, "generic_fallback":fallback,
        "owner_post_completion":owner_post, "remaining_action":second,
        "post_remaining_fallback":after_remaining, "owner_final":owner_final,
        "same_face_flat_comparison":comparison,
        "comparison_kind":"generic-fallback-versus-ordinary-flat-component",
        "stale_negative":{"proof_class":"deterministic-canonical-admission-on-observed-live-evidence",
            "refusal":format!("{stale_refusal:?}"), "installed_replay_enacted":false},
        "owner_refreshes":4, "native_mutations":2,
        "screenshots":[selected_image,fallback_image,after_image,comparison_image],
    }))
}

fn validate_fallback(selected: &Value, fallback: &Value) -> Result<(), ConduitosError> {
    let (_, old) = decode(selected)?;
    let (face, fresh) = decode(fallback)?;
    let cause = match conduit_face::prepare_component_recipe(recipe(fallback)?, &face) {
        Err(
            error @ (conduit_face::ComponentRecipeRefusal::SlotAction
            | conduit_face::ComponentRecipeRefusal::MissingSlot),
        ) => error,
        _ => return Err(refusal("native-todo-fallback-not-required-slot-refusal")),
    };
    if fallback["fallback"] != format!("{cause:?}")
        || fallback["configuration"] != selected["configuration"]
        || fallback["expression"] != selected["expression"]
        || fallback["face"] == selected["face"]
        || old.show_id == fresh.show_id
    {
        return Err(refusal("native-todo-reachable-fallback-invalid"));
    }
    Ok(())
}

fn validate_comparison(fallback: &Value, ordinary: &Value) -> Result<(), ConduitosError> {
    let (face, old) = decode(fallback)?;
    let (same, new) = decode(ordinary)?;
    if face != same
        || old.show_id == new.show_id
        || !ordinary["fallback"].is_null()
        || fallback["expression"] != ordinary["expression"]
        || fallback["configuration"]["recipe"] == ordinary["configuration"]["recipe"]
    {
        return Err(refusal("native-todo-fallback-comparison-not-same-face"));
    }
    Ok(())
}

fn recipe(value: &Value) -> Result<conduit_core::StructuredInfoValue, ConduitosError> {
    let configured: conduit_core::StructuredConfigurationValue =
        serde_json::from_value(value["configuration"]["recipe"].clone())
            .map_err(|_| refusal("native-todo-fallback-recipe-invalid"))?;
    conduit_core::StructuredInfoValue::from_canonical_bytes(configured.canonical_value())
        .map_err(|_| refusal("native-todo-fallback-recipe-invalid"))
}

fn decode(value: &Value) -> Result<(Face, MaskShow), ConduitosError> {
    let face: Face = serde_json::from_value(value["face"].clone())
        .map_err(|_| refusal("native-todo-fallback-face-invalid"))?;
    let show: MaskShow = serde_json::from_value(value["acknowledged_show"].clone())
        .map_err(|_| refusal("native-todo-fallback-show-invalid"))?;
    show.validate(&face)
        .map_err(|_| refusal("native-todo-fallback-show-invalid"))?;
    if value["owner_show_acknowledged"] != true
        || show.show.lifecycle != ManifestationLifecycle::Available
    {
        return Err(refusal("native-todo-fallback-unacknowledged"));
    }
    Ok((face, show))
}

fn guest(value: &Value) -> Result<Value, ConduitosError> {
    let (face, show) = decode(value)?;
    Ok(json!({"face_id":face.identity,"face_revision":face.revision,"show_id":show.show_id}))
}

fn select(
    qmp: &mut UnixStream,
    reader: &mut qmp::Reader,
    shift: bool,
    key: &str,
) -> Result<(), ConduitosError> {
    if shift {
        super::super::hid_qmp::send_named_keys(
            qmp,
            reader,
            &["shift", key],
            true,
            "required-item-select",
        )?;
        super::super::hid_qmp::send_named_keys(
            qmp,
            reader,
            &[key, "shift"],
            false,
            "required-item-select",
        )
    } else {
        journey_input::key_pair(qmp, reader, key, "required-item-flat-compare")
    }
}

#[allow(clippy::too_many_arguments)]
fn enact(
    tabs: u8,
    action: &str,
    prior: &Value,
    serial: &Path,
    child: &mut Child,
    qmp: &mut UnixStream,
    reader: &mut qmp::Reader,
) -> Result<(Value, Value), ConduitosError> {
    let count = action_records(serial)?.len();
    for _ in 0..tabs {
        journey_input::key_pair(qmp, reader, "tab", "required-item-focus")?;
    }
    journey_input::key_pair(qmp, reader, "ret", "required-item-submit")?;
    let deadline = Instant::now() + Duration::from_secs(30);
    loop {
        let records = action_records(serial)?;
        if records.len() > count {
            if records.len() != count + 1 {
                return Err(refusal("native-todo-fallback-duplicate-action"));
            }
            let receipt = records.last().unwrap();
            validate_action_receipt(receipt, action, prior)?;
            let next = wait_component(serial, child, &prior["acknowledged_show"]["show_id"])?;
            return Ok((receipt.clone(), next));
        }
        wait_or_refuse(child, deadline, "native-todo-fallback-action-timeout")?;
    }
}

fn validate_action_receipt(
    receipt: &Value,
    action: &str,
    prior: &Value,
) -> Result<(), ConduitosError> {
    if receipt["status"] != "accepted"
        || receipt["face_refreshed"] != true
        || receipt["action_id"] != action
        || receipt["prior_show_id"] != prior["acknowledged_show"]["show_id"]
        || receipt["face_id"] != prior["face"]["identity"]
        || receipt["face_revision"] != prior["face"]["revision"]
    {
        return Err(refusal("native-todo-fallback-action-mismatch"));
    }
    Ok(())
}

fn action_records(serial: &Path) -> Result<Vec<Value>, ConduitosError> {
    parse_records(&bounded_serial(serial)?, OWNER_ACTION)
}
fn latest(serial: &Path) -> Result<Value, ConduitosError> {
    parse_records(&bounded_serial(serial)?, COMPONENT)?
        .pop()
        .ok_or_else(|| refusal("native-todo-fallback-component-absent"))
}
fn wait_component(
    serial: &Path,
    child: &mut Child,
    prior: &Value,
) -> Result<Value, ConduitosError> {
    let deadline = Instant::now() + Duration::from_secs(120);
    loop {
        let record = latest(serial)?;
        if record["acknowledged_show"]["show_id"] != *prior {
            decode(&record)?;
            return Ok(record);
        }
        wait_or_refuse(child, deadline, "native-todo-fallback-ack-timeout")?;
    }
}
fn parse_records(text: &str, prefix: &str) -> Result<Vec<Value>, ConduitosError> {
    let mut result = Vec::new();
    for line in text
        .split_inclusive('\n')
        .filter(|line| line.ends_with('\n'))
    {
        if let Some(raw) = line.trim_end_matches('\n').strip_prefix(prefix) {
            if result.len() >= 24 || raw.len() > 65_536 {
                return Err(refusal("native-todo-fallback-record-bound"));
            }
            result.push(
                serde_json::from_str(raw)
                    .map_err(|_| refusal("native-todo-fallback-record-invalid"))?,
            );
        }
    }
    Ok(result)
}
fn capture(
    qmp: &mut UnixStream,
    reader: &mut qmp::Reader,
    directory: &Path,
    name: &str,
) -> Result<Value, ConduitosError> {
    let (image, health) = qmp_display::capture(qmp, reader, directory, name)?;
    if let Some(error) = health {
        return Err(error);
    }
    Ok(image)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn action_receipt_requires_the_exact_acknowledged_predecessor_and_refresh() {
        let prior = json!({"face":{"identity":"face/one","revision":2},
            "acknowledged_show":{"show_id":"show/one"}});
        let valid = json!({"status":"accepted","face_refreshed":true,
            "action_id":"todo.complete.task-1","prior_show_id":"show/one",
            "face_id":"face/one","face_revision":2});
        assert!(validate_action_receipt(&valid, "todo.complete.task-1", &prior).is_ok());
        for (field, wrong) in [
            ("prior_show_id", json!("show/old")),
            ("face_id", json!("face/old")),
            ("face_revision", json!(1)),
            ("status", json!("refused")),
            ("face_refreshed", json!(false)),
            ("action_id", json!("todo.complete.task-2")),
        ] {
            let mut bad = valid.clone();
            bad[field] = wrong;
            assert!(validate_action_receipt(&bad, "todo.complete.task-1", &prior).is_err());
        }
    }
    #[test]
    fn bounded_records_do_not_accept_truncated_or_unbounded_evidence() {
        assert!(parse_records("C {bad}\n", "C ").is_err());
        assert!(parse_records(&"C {}\n".repeat(25), "C ").is_err());
        assert!(parse_records("C {}", "C ").unwrap().is_empty());
        assert_eq!(parse_records("C {}\n", "C ").unwrap().len(), 1);
    }
}
