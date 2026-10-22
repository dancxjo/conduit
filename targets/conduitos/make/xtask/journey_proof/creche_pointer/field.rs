//! Physical pointer focus of a current argument field; never click-to-select.
use super::*;
use serde_json::json;

pub(in super::super) fn focus_argument(
    serial: &Path,
    child: &mut Child,
    stream: &mut UnixStream,
    reader: &mut qmp::Reader,
    action: &str,
    argument: &str,
) -> Result<Value, ConduitosError> {
    let target = (action, Some(argument));
    creche_choices::navigate_to(serial, child, stream, reader, action, Some(argument), false)?;
    let current = focus(serial, target)?;
    let source_before = records(serial, "CONDUIT_NATIVE_FACE ")?
        .pop()
        .ok_or_else(|| refusal("pointer field Source Face absent"))?;
    rectangle(&current)?;
    let changes = records(serial, CHANGE)?.len();
    let scale = (
        calibrate(serial, child, stream, reader, true, target)?,
        calibrate(serial, child, stream, reader, false, target)?,
    );
    move_to_hit(serial, child, stream, reader, scale, false, target)?;
    let origin = records(serial, POINTER)?
        .pop()
        .ok_or_else(|| refusal("field pointer origin absent"))?;
    if !within_hit(&origin, &current)? {
        return Err(refusal("field pointer not inside actual current bounds"));
    }
    // Move actual keyboard focus away, while requiring the same displayed page.
    // A field click must newly focus its argument, not merely preserve old focus.
    creche_choices::navigate_to(serial, child, stream, reader, action, None, false)?;
    let away = focus(serial, (action, None))?;
    if away["page"] != current["page"] || away["show_id"] != current["show_id"] {
        return Err(refusal(
            "naming action and argument lack a shared current pointer page",
        ));
    }
    let focus_before = records(serial, FOCUS)?.len();
    let after = last_sequence(serial)?;
    let observed = journey_input::primary_button(stream, reader, true, "creche-field-press")
        .and_then(|()| {
            let press = wait_sample(serial, child, after, |s| {
                s["primary_pressed"] == true && s["press_edge"] == true
            })?;
            bind(&press, &current)?;
            if !within_hit(&press, &current)? {
                return Err(refusal("physical field press outside current hit"));
            }
            journey_input::wait_for_record_retaining_guest(
                serial,
                child,
                "creche-field-focus-timeout",
                "argument-focus",
                |text| {
                    let focuses = face_correspondence::records(text, FOCUS)?;
                    Ok(focuses.len() > focus_before
                        && focuses.last().is_some_and(|f| {
                            f["action_id"] == action
                                && f["argument_name"] == argument
                                && f["show_id"] == current["show_id"]
                        }))
                },
            )?;
            let fresh = focus(serial, target)?;
            if fresh["face_id"] != current["face_id"]
                || fresh["show_id"] != current["show_id"]
                || records(serial, CHANGE)?.len() != changes
            {
                return Err(refusal(
                    "pointer field focus unexpectedly committed Source state",
                ));
            }
            Ok((press, fresh))
        });
    // Release even after refusal; this is not a retry or a semantic action.
    let prior = last_sequence(serial);
    let released = journey_input::primary_button(stream, reader, false, "creche-field-release")
        .and_then(|()| {
            let release = wait_sample(serial, child, prior?, |s| s["primary_pressed"] == false)?;
            bind(&release, &current)?;
            if release["press_edge"] != false {
                return Err(refusal("field release became a new press"));
            }
            Ok(release)
        });
    let (press, fresh) = observed?;
    let release = released?;
    // dispatch() emits a pointer observation before checking its semantic press
    // route. A later harmless released motion is a sequential guest barrier:
    // the event loop cannot observe it before returning from release dispatch.
    let barrier = move_by(serial, child, stream, reader, scale, (1, 0), false, target)?;
    if sequence(&barrier)? <= sequence(&release)? {
        return Err(refusal("post-release barrier is not later than release"));
    }
    let source_after = records(serial, "CONDUIT_NATIVE_FACE ")?
        .pop()
        .ok_or_else(|| refusal("post-release Source Face absent"))?;
    post_release_unchanged(
        &source_before,
        &source_after,
        changes,
        records(serial, CHANGE)?.len(),
    )?;
    let final_focus = focus(serial, target)?;
    if ["face_id", "face_revision", "show_id"]
        .iter()
        .any(|key| final_focus[*key] != current[*key])
    {
        return Err(refusal(
            "field release/barrier changed current Face or Show",
        ));
    }
    Ok(
        json!({"schema":"conduit.creche/argument-pointer-focus@1","action":action,"argument":argument,
        "target_geometry":current,"before_press_focus":away,"press":press,"after_focus":fresh,"release":release,"post_release_barrier":barrier,"final_focus":final_focus,
        "source_change_count":0,"selection_effect":"focus only; naming changes require checked keyboard commit","physical_evidence":false}),
    )
}

fn post_release_unchanged(
    before: &Value,
    after: &Value,
    original_changes: usize,
    final_changes: usize,
) -> Result<(), ConduitosError> {
    if original_changes != final_changes
        || before["face"] != after["face"]
        || before["show"] != after["show"]
    {
        return Err(refusal(
            "pointer release/barrier mutated Source or acknowledged Show",
        ));
    }
    Ok(())
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn release_only_effect_or_stale_show_cannot_be_absorbed_by_later_restoration() {
        let before = json!({"face":{"identity":"face","revision":1,"value":"original"},"show":{"show_id":"show","face_revision":1}});
        assert!(post_release_unchanged(&before, &before, 0, 0).is_ok());
        assert!(post_release_unchanged(&before, &before, 0, 1).is_err());
        let mut changed = before.clone();
        changed["face"]["value"] = json!("release-only-mutation");
        assert!(post_release_unchanged(&before, &changed, 0, 0).is_err());
        let mut stale = before.clone();
        stale["show"]["show_id"] = json!("stale-show");
        assert!(post_release_unchanged(&before, &stale, 0, 0).is_err());
    }
}
