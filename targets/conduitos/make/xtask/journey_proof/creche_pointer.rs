//! Real physical mouse gestures over observed current Crèche hit regions.
use super::super::{qemu_artifacts::Artifacts, qmp};
use super::{
    creche_choices, face_correspondence, io_error, journey_input, refusal, ConduitosError,
};
use serde_json::Value;
use std::{fs, os::unix::net::UnixStream, path::Path, process::Child};

mod evidence;
pub(super) use evidence::receipt;

const ACTION: &str = "creche.plot.3";
const POINTER: &str = "CONDUIT_NATIVE_POINTER ";
const FOCUS: &str = "CONDUIT_CRECHE_FOCUS ";
const CHANGE: &str = "CONDUIT_CRECHE_CHANGE ";

pub(super) fn exercise(
    serial: &Path,
    child: &mut Child,
    stream: &mut UnixStream,
    reader: &mut qmp::Reader,
    artifacts: &mut Artifacts,
) -> Result<(), ConduitosError> {
    creche_choices::navigate_to(serial, child, stream, reader, ACTION, None, false)?;
    let focus = focus(serial)?;
    if focus["hit_bounds"].is_null() {
        return Err(refusal("fourth checked Plot has no current frame hit"));
    }
    artifacts.capture(stream, reader, "creche-fourth-checked", true)?;
    // Calibrate physical report units independently on each axis. These are
    // motion only; no semantic action is sent or retried.
    let x = calibrate(serial, child, stream, reader, true)?;
    let y = calibrate(serial, child, stream, reader, false)?;
    for (expected, held, name) in [
        (false, true, "creche-pointer-three-selected"),
        (true, false, "creche-pointer-four-restored"),
    ] {
        move_to_hit(serial, child, stream, reader, (x, y), false)?;
        gesture(
            serial,
            child,
            stream,
            reader,
            artifacts,
            (x, y),
            expected,
            held,
            name,
        )?;
    }
    Ok(())
}

fn text(serial: &Path) -> Result<String, ConduitosError> {
    fs::read_to_string(serial).map_err(io_error)
}
fn records(serial: &Path, prefix: &str) -> Result<Vec<Value>, ConduitosError> {
    face_correspondence::records(&text(serial)?, prefix)
}
fn number(record: &Value, field: &str) -> Result<i64, ConduitosError> {
    record[field]
        .as_i64()
        .ok_or_else(|| refusal(format!("pointer observation missing {field}")))
}
fn sequence(record: &Value) -> Result<u64, ConduitosError> {
    record["sequence"]
        .as_u64()
        .ok_or_else(|| refusal("pointer observation lacks sequence"))
}
fn last_sequence(serial: &Path) -> Result<u64, ConduitosError> {
    records(serial, POINTER)?
        .last()
        .map(sequence)
        .transpose()
        .map(|value| value.unwrap_or(0))
}
fn physical(record: Value) -> Result<Value, ConduitosError> {
    if record["schema"] != "conduit.native-pointer/v1"
        || record["coalesced"] != 0
        || record["dropped"] != 0
        || record["primary_pressed"].as_bool().is_none()
        || record["press_edge"].as_bool().is_none()
        || !(0..=1_000_000).contains(&number(&record, "position_x")?)
        || !(0..=1_000_000).contains(&number(&record, "position_y")?)
    {
        return Err(refusal(
            "physical pointer observation is invalid or has a history gap",
        ));
    }
    sequence(&record)?;
    Ok(record)
}
fn focus(serial: &Path) -> Result<Value, ConduitosError> {
    let observations = records(serial, FOCUS)?;
    let focus = observations
        .last()
        .ok_or_else(|| refusal("missing actual pointer target focus"))?;
    let faces = records(serial, "CONDUIT_NATIVE_FACE ")?;
    let face = faces
        .last()
        .ok_or_else(|| refusal("missing acknowledged pointer target Face"))?;
    if focus["schema"] != "conduit.creche-focus/v1"
        || focus["action_id"] != ACTION
        || !focus["argument_name"].is_null()
        || focus["face_id"] != face["face"]["identity"]
        || focus["face_revision"] != face["face"]["revision"]
        || focus["show_id"] != face["show"]["show_id"]
    {
        return Err(refusal(
            "pointer target is not the observed fourth control on the current Show",
        ));
    }
    Ok(focus.clone())
}
fn bind(record: &Value, focus: &Value) -> Result<(), ConduitosError> {
    if ["face_id", "face_revision", "show_id"]
        .iter()
        .any(|field| record[*field] != focus[*field])
    {
        return Err(refusal(
            "physical pointer sample does not bind the current target Show",
        ));
    }
    Ok(())
}
fn wait_sample(
    serial: &Path,
    child: &mut Child,
    after: u64,
    mut matches: impl FnMut(&Value) -> bool,
) -> Result<Value, ConduitosError> {
    let mut observed = None;
    journey_input::wait_for_record_retaining_guest(
        serial,
        child,
        "creche-physical-pointer-timeout",
        "physical-pointer",
        |text| {
            let samples = face_correspondence::records(text, POINTER)?;
            observed = samples.into_iter().rev().find(|sample| {
                sample["sequence"]
                    .as_u64()
                    .is_some_and(|value| value > after)
                    && matches(sample)
            });
            Ok(observed.is_some())
        },
    )?;
    physical(observed.ok_or_else(|| refusal("physical pointer observation disappeared"))?)
}
fn calibrate(
    serial: &Path,
    child: &mut Child,
    stream: &mut UnixStream,
    reader: &mut qmp::Reader,
    horizontal: bool,
) -> Result<i64, ConduitosError> {
    let prior = last_sequence(serial)?;
    journey_input::relative_motion(
        stream,
        reader,
        i64::from(horizontal),
        i64::from(!horizontal),
        "creche-pointer-calibrate",
    )?;
    let axis = if horizontal { "delta_x" } else { "delta_y" };
    let sample = wait_sample(serial, child, prior, |sample| {
        sample[axis].as_i64().is_some_and(|delta| delta > 0)
    })?;
    bind(&sample, &focus(serial)?)?;
    if sample["primary_pressed"] != false || sample["press_edge"] != false {
        return Err(refusal(
            "pointer calibration unexpectedly pressed a control",
        ));
    }
    let scale = number(&sample, axis)?;
    if !(1..=1_000_000).contains(&scale) {
        return Err(refusal(
            "physical pointer scale is outside normalized extent",
        ));
    }
    Ok(scale)
}

fn relative_delta(target: i64, current: i64, scale: i64) -> i64 {
    let difference = target - current;
    let rounded = (difference.abs() + scale / 2) / scale;
    (rounded * difference.signum()).clamp(-127, 127)
}
fn rectangle(focus: &Value) -> Result<(i64, i64, i64, i64, i64, i64), ConduitosError> {
    let hit = &focus["hit_bounds"];
    let extent = &focus["display_bounds"];
    let (x, y, width, height) = (
        number(hit, "x")?,
        number(hit, "y")?,
        number(hit, "width")?,
        number(hit, "height")?,
    );
    let (display_width, display_height) = (number(extent, "width")?, number(extent, "height")?);
    if number(extent, "x")? != 0
        || number(extent, "y")? != 0
        || x < 0
        || y < 0
        || width <= 0
        || height <= 0
        || display_width > 16_384
        || display_height > 16_384
        || x.checked_add(width).is_none_or(|end| end > display_width)
        || y.checked_add(height).is_none_or(|end| end > display_height)
    {
        return Err(refusal("observed pointer hit exceeds its admitted display"));
    }
    Ok((x, y, width, height, display_width, display_height))
}
fn within_hit(sample: &Value, focus: &Value) -> Result<bool, ConduitosError> {
    let (x, y, width, height, _, _) = rectangle(focus)?;
    let px = number(sample, "display_x")?;
    let py = number(sample, "display_y")?;
    Ok(px >= x && px < x + width && py >= y && py < y + height)
}

fn move_by(
    serial: &Path,
    child: &mut Child,
    stream: &mut UnixStream,
    reader: &mut qmp::Reader,
    scale: (i64, i64),
    delta: (i64, i64),
    pressed: bool,
) -> Result<Value, ConduitosError> {
    let samples = records(serial, POINTER)?;
    let prior = samples
        .last()
        .ok_or_else(|| refusal("pointer motion lacks an observed origin"))?;
    let after = sequence(prior)?;
    let x = (number(prior, "position_x")? + delta.0 * scale.0).clamp(0, 1_000_000);
    let y = (number(prior, "position_y")? + delta.1 * scale.1).clamp(0, 1_000_000);
    journey_input::relative_motion(stream, reader, delta.0, delta.1, "creche-pointer-motion")?;
    let sample = wait_sample(serial, child, after, |sample| {
        sample["position_x"] == x
            && sample["position_y"] == y
            && sample["primary_pressed"] == pressed
    })?;
    bind(&sample, &focus(serial)?)?;
    if sample["press_edge"] != false {
        return Err(refusal("motion unexpectedly became a new physical press"));
    }
    Ok(sample)
}
fn move_to_hit(
    serial: &Path,
    child: &mut Child,
    stream: &mut UnixStream,
    reader: &mut qmp::Reader,
    scale: (i64, i64),
    pressed: bool,
) -> Result<(), ConduitosError> {
    let focus = focus(serial)?;
    let (x, y, width, height, display_width, display_height) = rectangle(&focus)?;
    // Four bounded motion reports suffice for the full normalized span with
    // the observed boot-pointer scale. An unsuitable scale refuses explicitly.
    for _ in 0..4 {
        let samples = records(serial, POINTER)?;
        let current = samples
            .last()
            .ok_or_else(|| refusal("pointer origin observation absent"))?;
        bind(current, &focus)?;
        if current["primary_pressed"] != pressed {
            return Err(refusal("pointer positioning changed the held button state"));
        }
        if within_hit(current, &focus)? {
            return Ok(());
        }
        let target_x = (x + width / 2) * 1_000_000 / display_width;
        let target_y = (y + height / 2) * 1_000_000 / display_height;
        let delta = (
            relative_delta(target_x, number(current, "position_x")?, scale.0),
            relative_delta(target_y, number(current, "position_y")?, scale.1),
        );
        if delta == (0, 0) {
            break;
        }
        move_by(serial, child, stream, reader, scale, delta, pressed)?;
    }
    Err(refusal(
        "actual pointer did not reach current hit within the finite motion bound",
    ))
}

#[allow(clippy::too_many_arguments)]
fn gesture(
    serial: &Path,
    child: &mut Child,
    stream: &mut UnixStream,
    reader: &mut qmp::Reader,
    artifacts: &mut Artifacts,
    scale: (i64, i64),
    expected: bool,
    held: bool,
    name: &str,
) -> Result<(), ConduitosError> {
    let before = records(serial, CHANGE)?.len();
    let prior_sequence = last_sequence(serial)?;
    let observed = journey_input::primary_button(stream, reader, true, "creche-pointer-press")
        .and_then(|()| {
            journey_input::wait_for_record_retaining_guest(
                serial,
                child,
                "creche-pointer-change-timeout",
                name,
                |text| Ok(face_correspondence::records(text, CHANGE)?.len() > before),
            )?;
            let serial_text = text(serial)?;
            let change = creche_choices::validate(&serial_text, ACTION, expected)?;
            if records(serial, CHANGE)?.len() != before + 1 {
                return Err(refusal(
                    "one physical press produced more than one Source edit",
                ));
            }
            let samples = records(serial, POINTER)?;
            let presses = samples
                .iter()
                .filter(|sample| {
                    sample["sequence"]
                        .as_u64()
                        .is_some_and(|sequence| sequence > prior_sequence)
                        && sample["primary_pressed"] == true
                        && sample["press_edge"] == true
                        && sample["face_id"] == change["before_face_id"]
                        && sample["show_id"] == change["before_show_id"]
                })
                .count();
            if presses != 1 {
                return Err(refusal(
                    "accepted Source edit lacks one exact physical press on its prior Show",
                ));
            }
            if held {
                // First observe the still-held cursor under the fresh Show,
                // then move onto its current hit without releasing the button.
                move_by(serial, child, stream, reader, scale, (1, 0), true)?;
                move_to_hit(serial, child, stream, reader, scale, true)?;
                let sample = records(serial, POINTER)?
                    .last()
                    .cloned()
                    .ok_or_else(|| refusal("held cursor observation absent"))?;
                if !within_hit(&sample, &focus(serial)?)?
                    || sample["press_edge"] != false
                    || records(serial, CHANGE)?.len() != before + 1
                {
                    return Err(refusal(
                        "held motion after fresh Show repeated the Source action",
                    ));
                }
                creche_choices::validate(&text(serial)?, ACTION, expected)?;
            }
            artifacts.capture(stream, reader, name, true)?;
            Ok(())
        });
    // Always release this one gesture, including on an observation refusal.
    // No semantic action is replayed to recover a missing receipt.
    let after = last_sequence(serial);
    let released = journey_input::primary_button(stream, reader, false, "creche-pointer-release")
        .and_then(|()| {
            let sample = wait_sample(serial, child, after?, |sample| {
                sample["primary_pressed"] == false
            })?;
            bind(&sample, &focus(serial)?)?;
            if sample["press_edge"] != false {
                return Err(refusal("release became an activation"));
            }
            Ok(())
        });
    observed.and(released)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    #[test]
    fn motion_rounds_in_both_directions_and_keeps_boot_report_bounds() {
        assert_eq!(relative_delta(900_000, 500_000, 4_000), 100);
        assert_eq!(relative_delta(100_000, 500_000, 4_000), -100);
        assert_eq!(relative_delta(1_000_000, 0, 4_000), 127);
        assert_eq!(relative_delta(0, 1_000_000, 4_000), -127);
        assert_eq!(relative_delta(500_000, 500_000, 4_000), 0);
    }
    #[test]
    fn observed_hit_must_fit_the_actual_admitted_display() {
        let mut focus = json!({"hit_bounds":{"x":10,"y":20,"width":100,"height":30},
            "display_bounds":{"x":0,"y":0,"width":640,"height":480}});
        assert!(rectangle(&focus).is_ok());
        focus["hit_bounds"]["width"] = json!(641);
        assert!(rectangle(&focus).is_err());
        focus["hit_bounds"]["width"] = json!(0);
        assert!(rectangle(&focus).is_err());
    }
    #[test]
    fn held_probe_must_enter_the_refreshed_control_not_its_old_location() {
        let focus = json!({"hit_bounds":{"x":60,"y":56,"width":1188,"height":33},
            "display_bounds":{"x":0,"y":0,"width":1280,"height":800}});
        assert!(!within_hit(&json!({"display_x":660,"display_y":156}), &focus).unwrap());
        assert!(within_hit(&json!({"display_x":660,"display_y":70}), &focus).unwrap());
    }
}
