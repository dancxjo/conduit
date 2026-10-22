//! Actual native focus traversal over the owner's unchanged Face and Show.
use super::*;
use conduit_face::FaceActionAvailability;
const FOCUS: &str = "CONDUIT_CRECHE_FOCUS ";
const MAX_FOCUS_RECORDS: usize = 4_096;

#[derive(PartialEq, Eq)]
struct Control {
    action: String,
    argument: Option<String>,
}
fn control(value: &Value) -> Result<Control, ConduitosError> {
    Ok(Control {
        action: value["action_id"]
            .as_str()
            .ok_or_else(|| refusal("native-look-focus-action-absent"))?
            .into(),
        argument: match &value["argument_name"] {
            Value::Null => None,
            Value::String(value) => Some(value.clone()),
            _ => return Err(refusal("native-look-focus-argument-invalid")),
        },
    })
}
fn records(serial: &Path) -> Result<Vec<Value>, ConduitosError> {
    let mut result = Vec::new();
    for line in bounded_serial(serial)?
        .split_inclusive('\n')
        .filter(|line| line.ends_with('\n'))
    {
        if let Some(raw) = line.trim_end_matches('\n').strip_prefix(FOCUS) {
            if result.len() == MAX_FOCUS_RECORDS || raw.len() > 4_096 {
                return Err(refusal("native-look-focus-record-bound"));
            }
            result.push(
                serde_json::from_str(raw)
                    .map_err(|_| refusal("native-look-focus-record-invalid"))?,
            );
        }
    }
    Ok(result)
}
fn validate(value: &Value, face: &Face, show: &Value) -> Result<(), ConduitosError> {
    let selected = control(value)?;
    let action = face
        .actions
        .iter()
        .find(|action| action.identity == selected.action)
        .ok_or_else(|| refusal("native-look-focus-unknown-action"))?;
    let pages = value["pages"].as_u64().filter(|pages| *pages > 0);
    let page = value["page"].as_u64();
    let hit = &value["hit_bounds"];
    let display = &value["display_bounds"];
    let fits = |axis: &str, extent: &str| -> bool {
        let Some(start) = hit[axis].as_u64() else {
            return false;
        };
        let Some(length) = hit[extent].as_u64().filter(|value| *value > 0) else {
            return false;
        };
        let Some(origin) = display[axis].as_u64() else {
            return false;
        };
        let Some(capacity) = display[extent].as_u64() else {
            return false;
        };
        start >= origin
            && start
                .checked_add(length)
                .zip(origin.checked_add(capacity))
                .is_some_and(|(end, bound)| end <= bound)
    };
    if value["face_id"] != json!(face.identity)
        || value["face_revision"] != face.revision
        || value["show_id"] != *show
        || !action.availability.is_available()
        || selected.argument.as_ref().is_some_and(|name| {
            !action
                .arguments
                .iter()
                .any(|argument| &argument.name == name)
        })
        || !page
            .zip(pages)
            .is_some_and(|(page, pages)| page > 0 && page <= pages)
        || !fits("x", "width")
        || !fits("y", "height")
    {
        return Err(refusal("native-look-focus-not-current-and-reachable"));
    }
    Ok(())
}
fn key(
    serial: &Path,
    child: &mut Child,
    stream: &mut UnixStream,
    reader: &mut qmp::Reader,
    key: &str,
    face: &Face,
    show: &Value,
) -> Result<Value, ConduitosError> {
    let count = records(serial)?.len();
    let deadline = Instant::now() + Duration::from_secs(20);
    let mut observed = None;
    journey_input::held_key(
        |down| hid_qmp::send_named_keys(stream, reader, &[key], down, "todo-look-focus"),
        || loop {
            let values = records(serial)?;
            if values.len() > count {
                if values.len() != count + 1 {
                    return Err(refusal("native-look-duplicate-focus"));
                }
                let value = values.last().unwrap();
                validate(value, face, show)?;
                observed = Some(value.clone());
                return Ok(());
            }
            wait_or_refuse(child, deadline, "native-look-focus-timeout")?;
        },
    )?;
    observed.ok_or_else(|| refusal("native-look-focus-observation-absent"))
}
pub(super) fn initial(
    serial: &Path,
    child: &mut Child,
    stream: &mut UnixStream,
    reader: &mut qmp::Reader,
    face: &Face,
    show: &Value,
) -> Result<Value, ConduitosError> {
    key(serial, child, stream, reader, "tab", face, show)
}
pub(super) fn retained(
    serial: &Path,
    child: &mut Child,
    face: &Face,
    show: &Value,
    anchor: &Value,
) -> Result<Value, ConduitosError> {
    let deadline = Instant::now() + Duration::from_secs(20);
    loop {
        if let Some(value) = records(serial)?
            .last()
            .filter(|value| value["show_id"] == *show)
        {
            validate(value, face, show)?;
            if control(value)? != control(anchor)? {
                return Err(refusal("native-look-focus-not-retained"));
            }
            return Ok(value.clone());
        }
        wait_or_refuse(child, deadline, "native-look-retained-focus-timeout")?;
    }
}
fn ring(
    serial: &Path,
    child: &mut Child,
    stream: &mut UnixStream,
    reader: &mut qmp::Reader,
    face: &Face,
    show: &Value,
    anchor: &Value,
) -> Result<Vec<Value>, ConduitosError> {
    let start = control(anchor)?;
    let bound = face
        .actions
        .iter()
        .filter(|action| action.availability == FaceActionAvailability::Available)
        .map(|action| 1 + action.arguments.len())
        .sum::<usize>();
    let mut values = vec![anchor.clone()];
    for _ in 0..=bound {
        let value = key(serial, child, stream, reader, "tab", face, show)?;
        let next = control(&value)?;
        if next == start {
            return Ok(values);
        }
        if values
            .iter()
            .any(|previous| control(previous).ok().as_ref() == Some(&next))
        {
            return Err(refusal("native-look-focus-repeated-before-complete-ring"));
        }
        values.push(value);
    }
    Err(refusal("native-look-focus-traversal-bound"))
}
pub(super) fn traverse(
    serial: &Path,
    child: &mut Child,
    stream: &mut UnixStream,
    reader: &mut qmp::Reader,
    face: &Face,
    show: &Value,
    anchor: &Value,
) -> Result<Value, ConduitosError> {
    let primary = ring(serial, child, stream, reader, face, show, anchor)?;
    let detail_anchor = key(serial, child, stream, reader, "f2", face, show)?;
    let details = ring(serial, child, stream, reader, face, show, &detail_anchor)?;
    let mut expected = Vec::new();
    for action in face
        .actions
        .iter()
        .filter(|action| action.availability.is_available())
    {
        expected.push(Control {
            action: action.identity.clone(),
            argument: None,
        });
        for argument in &action.arguments {
            expected.push(Control {
                action: action.identity.clone(),
                argument: Some(argument.name.clone()),
            });
        }
    }
    if details.len() != expected.len()
        || expected.iter().any(|expected| {
            !details
                .iter()
                .any(|value| control(value).ok().as_ref() == Some(expected))
        })
    {
        return Err(refusal("native-look-details-lost-offered-controls"));
    }
    let returned = key(serial, child, stream, reader, "f2", face, show)?;
    if control(&returned)? != control(anchor)? {
        return Err(refusal("native-look-reading-lost-anchor"));
    }
    Ok(json!({"primary":primary,"details":details,"returned":returned}))
}
pub(super) fn same_order(before: &Value, after: &Value) -> Result<(), ConduitosError> {
    for view in ["primary", "details"] {
        let left = before[view]
            .as_array()
            .ok_or_else(|| refusal("native-look-focus-ring-absent"))?;
        let right = after[view]
            .as_array()
            .ok_or_else(|| refusal("native-look-focus-ring-absent"))?;
        if left.len() != right.len()
            || left
                .iter()
                .zip(right)
                .any(|(left, right)| control(left).ok() != control(right).ok())
        {
            return Err(refusal("native-look-swap-changed-action-order"));
        }
    }
    Ok(())
}
