//! Four actual preference requests through one finite installed-owner return route.
use super::super::hid_qmp;
use super::*;
use conduit_face::{Face, ManifestationLifecycle, MaskShow};
use std::{os::unix::net::UnixStream, path::Path, time::Instant};

const COMPONENT: &str = "CONDUIT_NATIVE_COMPONENT_REALIZATION ";

pub(super) fn validate_installation(
    args: &LiveOwnerTodoFaceProofArgs,
    source: &str,
) -> Result<(), ConduitosError> {
    let state = fs::canonicalize(&args.owner_state_dir)
        .map_err(|_| refusal("native-todo-look-owner-state-absent"))?;
    let bin = fs::canonicalize(&args.owner_conduit_bin)
        .map_err(|_| refusal("native-todo-look-owner-bin-absent"))?;
    let path = state.join("installation.json");
    let metadata =
        fs::metadata(&path).map_err(|_| refusal("native-todo-look-installation-absent"))?;
    if !metadata.is_file() || metadata.len() > 512 * 1024 || !bin.is_file() {
        return Err(refusal("native-todo-look-installation-invalid"));
    }
    let installation: Value = serde_json::from_slice(
        &fs::read(path).map_err(|_| refusal("native-todo-look-installation-absent"))?,
    )
    .map_err(|_| refusal("native-todo-look-installation-invalid"))?;
    if !installation_matches(&installation, source, &bin) {
        return Err(refusal("native-todo-look-installation-source-mismatch"));
    }
    Ok(())
}

fn installation_matches(value: &Value, source: &str, bin: &Path) -> bool {
    value["release_source_identity"] == source
        && value["product_executable"] == bin.to_string_lossy().as_ref()
}

#[expect(
    clippy::too_many_arguments,
    reason = "one live owner/guest proof carries exact observations"
)]
pub(super) fn exercise(
    args: &LiveOwnerTodoFaceProofArgs,
    directory: &Path,
    serial: &Path,
    child: &mut Child,
    stream: &mut UnixStream,
    reader: &mut qmp::Reader,
    before: &Value,
) -> Result<Value, ConduitosError> {
    let owner_before = todo_validation::read_and_validate(args, before)?;
    let original: Face = serde_json::from_value(owner_before["face"].clone())
        .map_err(|_| refusal("native-todo-look-owner-face-invalid"))?;
    original
        .validate()
        .map_err(|_| refusal("native-todo-look-owner-face-invalid"))?;
    let mut previous = before["show_id"].clone();
    let mut seen = vec![previous.clone()];
    let mut origin: Option<Value> = None;
    let mut prior_show: Option<MaskShow> = None;
    let mut transitions = Vec::new();
    for (key, density, palette, scale, checkpoint) in [
        ("f7", "compact", "editorial", 100, "todo-editorial-compact"),
        (
            "f6",
            "spacious",
            "editorial",
            125,
            "todo-editorial-spacious",
        ),
        (
            "f7",
            "spacious",
            "phosphor",
            125,
            "todo-engineering-spacious",
        ),
        ("f6", "compact", "phosphor", 100, "todo-engineering-compact"),
    ] {
        let count = bounded_records(&bounded_serial(serial)?, COMPONENT, 65_536)?.len();
        hid_qmp::send_named_keys(stream, reader, &["shift", key], true, "todo-look-select")?;
        hid_qmp::send_named_keys(stream, reader, &[key, "shift"], false, "todo-look-select")?;
        let deadline = Instant::now() + Duration::from_secs(120);
        let record = loop {
            let text = bounded_serial(serial)?;
            let components = bounded_records(&text, COMPONENT, 65_536)?;
            if components.len() > count {
                if components.len() != count + 1 {
                    return Err(refusal("native-todo-look-duplicate-selection"));
                }
                let value = components.last().unwrap();
                let faces = bounded_records(&text, OWNER_FACE, 1_024)?;
                if faces.iter().any(|ack| {
                    ack["status"] == "acknowledged"
                        && ack["show_id"] == value["acknowledged_show"]["show_id"]
                }) {
                    break value.clone();
                }
            }
            wait_or_refuse(child, deadline, "native-todo-look-ack-timeout")?;
        };
        let face: Face = serde_json::from_value(record["face"].clone())
            .map_err(|_| refusal("native-todo-look-face-invalid"))?;
        let show: MaskShow = serde_json::from_value(record["acknowledged_show"].clone())
            .map_err(|_| refusal("native-todo-look-show-invalid"))?;
        show.validate(&face)
            .map_err(|_| refusal("native-todo-look-show-invalid"))?;
        let id = json!(show.show_id);
        let config = &record["configuration"];
        let expression = &record["expression"]["expression"];
        if record["schema"] != "conduit.conduitos/component-realization@1"
            || record["owner_show_acknowledged"] != true
            || !record["fallback"].is_null()
            || face != original
            || seen.contains(&id)
            || show.show.lifecycle != ManifestationLifecycle::Available
            || expression["density"] != density
            || expression["palette"] != palette
            || expression["text_scale_percent"] != scale
            || config["component"].is_null()
            || config["recipe"].is_null()
            || config["host_id"] != json!(show.show.host_id)
            || config["boot_id"] != json!(show.show.boot_id)
            || origin.as_ref().is_some_and(|first| {
                config["component"] != first["configuration"]["component"]
                    || config["recipe"] != first["configuration"]["recipe"]
                    || record["owner_host"] != first["owner_host"]
            })
            || prior_show.as_ref().is_some_and(|prior| {
                show.show.plan_id == prior.show.plan_id
                    || show.show.host_id != prior.show.host_id
                    || show.show.boot_id != prior.show.boot_id
                    || show.show.offer_generation != prior.show.offer_generation
            })
        {
            return Err(refusal("native-todo-look-correspondence-invalid"));
        }
        let text = bounded_serial(serial)?;
        if !bounded_records(&text, OWNER_ACTION, 1_024)?.is_empty() {
            return Err(refusal("native-todo-look-dispatched-body-action"));
        }
        let owner_now = todo_validation::read_and_validate(
            args,
            &json!({
                "face_id":face.identity, "face_revision":face.revision,
            }),
        )?;
        if owner_now["face"] != owner_before["face"] {
            return Err(refusal("native-todo-look-owner-state-changed"));
        }
        let (screenshot, health) = qmp_display::capture(stream, reader, directory, checkpoint)?;
        if let Some(error) = health {
            return Err(error);
        }
        if origin.is_none() {
            origin = Some(record.clone());
        }
        transitions.push(json!({"previous_show_id":previous,"realization":record,
            "owner_face":owner_now,"screenshot":screenshot}));
        previous = id.clone();
        seen.push(id);
        prior_show = Some(show);
    }
    Ok(
        json!({"schema":"conduit.conduitos/native-owner-todo-look-proof@1",
        "status":"passed","input":"real-qmp-shift-function-keys","owner_before":owner_before,
        "transitions":transitions,"body_actions":0,"mutations":0,
        "human_enactment":false,"physical_evidence":false}),
    )
}

fn bounded_records(
    serial: &str,
    prefix: &str,
    maximum_bytes: usize,
) -> Result<Vec<Value>, ConduitosError> {
    let mut records = Vec::new();
    for line in serial
        .split_inclusive('\n')
        .filter(|line| line.ends_with('\n'))
    {
        if let Some(value) = line.trim_end_matches('\n').strip_prefix(prefix) {
            if records.len() == 24 || value.len() > maximum_bytes {
                return Err(refusal("native-todo-look-record-pressure"));
            }
            records.push(
                serde_json::from_str(value)
                    .map_err(|_| refusal("native-todo-look-record-invalid"))?,
            );
        }
    }
    Ok(records)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn owner_look_requires_the_installed_product_from_the_exact_guest_source() {
        let value = json!({"release_source_identity":"source/actual", "product_executable":"/private/owner/conduit"});
        assert!(installation_matches(
            &value,
            "source/actual",
            Path::new("/private/owner/conduit")
        ));
        assert!(!installation_matches(
            &value,
            "source/stale",
            Path::new("/private/owner/conduit")
        ));
        assert!(!installation_matches(
            &value,
            "source/actual",
            Path::new("/fixture/conduit")
        ));
    }
    #[test]
    fn owner_look_records_wait_for_completion_and_refuse_pressure_or_corruption() {
        let line = format!("{COMPONENT}{{\"schema\":\"example\"}}\n");
        assert_eq!(
            bounded_records(&(line.clone() + COMPONENT + "{\"pending"), COMPONENT, 64)
                .unwrap()
                .len(),
            1
        );
        assert!(bounded_records(&line.repeat(25), COMPONENT, 64).is_err());
        assert!(bounded_records(&line, COMPONENT, 4).is_err());
        assert!(bounded_records(&format!("{COMPONENT}not-json\n"), COMPONENT, 64).is_err());
    }
}
